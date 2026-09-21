//! Capturing what a window looks like, through the Wayland protocols meant
//! for exactly that.
//!
//! `ext-image-copy-capture-v1` asks the compositor to copy a capture source
//! into a buffer the client provides; `ext-image-capture-source-v1` turns a
//! window into such a source; and `ext-foreign-toplevel-list-v1` is where the
//! windows come from. All three are standard, so nothing here is specific to
//! Hyprland beyond the identifier used to find a window, which the compositor
//! integration supplies.

use std::collections::HashMap;
use std::os::fd::AsFd;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use memmap2::MmapMut;
use rustix::event::{PollFd, PollFlags, Timespec};
use rustix::fs::{MemfdFlags, memfd_create};
use wayland_client::protocol::{wl_buffer, wl_registry, wl_shm, wl_shm_pool};
use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle, delegate_noop, event_created_child,
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::{
    self, ExtForeignToplevelHandleV1,
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_list_v1::{
    self, ExtForeignToplevelListV1,
};
use wayland_protocols::ext::image_capture_source::v1::client::ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1;
use wayland_protocols::ext::image_capture_source::v1::client::ext_image_capture_source_v1::ExtImageCaptureSourceV1;
use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_frame_v1::{
    self, ExtImageCopyCaptureFrameV1,
};
use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_manager_v1::{
    self, ExtImageCopyCaptureManagerV1,
};
use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_session_v1::{
    self, ExtImageCopyCaptureSessionV1,
};

use super::Thumbnail;

/// How long to wait for the compositor to hand over one window's contents.
/// Generous: a capture that takes this long has gone wrong, and the switcher
/// carries on without it either way.
const TIMEOUT: Duration = Duration::from_millis(500);

/// A Wayland connection of raisin's own, used only to capture windows.
pub(crate) struct Capturer {
    connection: Connection,
    queue: EventQueue<State>,
    state: State,
}

impl Capturer {
    /// Connects and finds out whether this compositor can capture windows at
    /// all.
    pub(crate) fn connect() -> Result<Self> {
        let connection =
            Connection::connect_to_env().context("no Wayland display to connect to")?;
        let mut queue = connection.new_event_queue();

        connection.display().get_registry(&queue.handle(), ());

        let mut state = State::default();
        // The first pass brings the globals, the second the windows they
        // advertise.
        queue
            .roundtrip(&mut state)
            .context("Wayland handshake failed")?;
        queue
            .roundtrip(&mut state)
            .context("Wayland handshake failed")?;

        anyhow::ensure!(
            state.shm.is_some() && state.sources.is_some() && state.captures.is_some(),
            "this compositor doesn't offer ext-image-copy-capture-v1 for windows"
        );

        Ok(Self {
            connection,
            queue,
            state,
        })
    }

    /// Catches up with windows that have opened or closed.
    pub(crate) fn refresh(&mut self) -> Result<()> {
        self.queue
            .roundtrip(&mut self.state)
            .context("lost the Wayland connection")?;

        Ok(())
    }

    /// Captures one window and scales it down to `width` pixels across.
    pub(crate) fn capture(&mut self, identifier: &str, width: u32) -> Result<Thumbnail> {
        let handle = self
            .state
            .windows
            .get(identifier)
            .cloned()
            .with_context(|| format!("the compositor doesn't list a window {identifier}"))?;

        let queue = self.queue.handle();
        let source = self
            .state
            .sources
            .as_ref()
            .context("no capture source manager")?
            .create_source(&handle, &queue, ());
        let session = self
            .state
            .captures
            .as_ref()
            .context("no capture manager")?
            .create_session(
                &source,
                ext_image_copy_capture_manager_v1::Options::empty(),
                &queue,
                (),
            );

        let captured = self.capture_into(&session, identifier, width);

        session.destroy();
        source.destroy();

        captured
    }

    fn capture_into(
        &mut self,
        session: &ExtImageCopyCaptureSessionV1,
        identifier: &str,
        width: u32,
    ) -> Result<Thumbnail> {
        self.state.frame = Frame::default();

        // The session first describes what it will hand over: how big, and in
        // which formats.
        self.dispatch_until(|state| state.frame.described || state.frame.stopped)?;
        anyhow::ensure!(
            !self.state.frame.stopped,
            "the compositor stopped capturing"
        );

        let (source_width, source_height) = self
            .state
            .frame
            .size
            .context("the compositor never said how big the window is")?;
        let format = self.state.frame.format().context("no format in common")?;
        let stride = source_width * 4;
        let size = stride * source_height;

        let (buffer, pool, memory) =
            self.buffer(source_width, source_height, stride, size, format)?;

        let frame = session.create_frame(&self.queue.handle(), ());
        frame.attach_buffer(&buffer);
        // Nothing of this buffer has been captured before, so all of it is
        // out of date.
        frame.damage_buffer(0, 0, source_width as i32, source_height as i32);
        frame.capture();

        let captured = self
            .dispatch_until(|state| state.frame.ready || state.frame.failed.is_some())
            .and_then(|()| match &self.state.frame.failed {
                Some(reason) => anyhow::bail!("the compositor refused to capture: {reason}"),
                None => Ok(super::scale(
                    identifier,
                    &memory,
                    source_width,
                    source_height,
                    stride,
                    width,
                    matches!(format, wl_shm::Format::Xrgb8888),
                )),
            });

        frame.destroy();
        buffer.destroy();
        pool.destroy();

        captured
    }

    /// Shared memory for the compositor to copy the window into.
    fn buffer(
        &self,
        width: u32,
        height: u32,
        stride: u32,
        size: u32,
        format: wl_shm::Format,
    ) -> Result<(wl_buffer::WlBuffer, wl_shm_pool::WlShmPool, MmapMut)> {
        let file = memfd_create("raisin-preview", MemfdFlags::CLOEXEC)
            .context("failed to make shared memory for a preview")?;
        rustix::fs::ftruncate(&file, u64::from(size)).context("failed to size shared memory")?;

        // SAFETY: the file was just created here, is not shared with anything
        // else yet, and is only written by the compositor while raisin waits.
        let memory = unsafe { MmapMut::map_mut(&file) }.context("failed to map shared memory")?;

        let queue = self.queue.handle();
        let pool = self.state.shm.as_ref().context("no wl_shm")?.create_pool(
            file.as_fd(),
            size as i32,
            &queue,
            (),
        );
        let buffer = pool.create_buffer(
            0,
            width as i32,
            height as i32,
            stride as i32,
            format,
            &queue,
            (),
        );

        Ok((buffer, pool, memory))
    }

    /// Reads Wayland events until `ready` says the wait is over, or until the
    /// compositor has had long enough.
    fn dispatch_until(&mut self, ready: impl Fn(&State) -> bool) -> Result<()> {
        let deadline = Instant::now() + TIMEOUT;

        while !ready(&self.state) {
            self.queue.flush().context("lost the Wayland connection")?;

            if self
                .queue
                .dispatch_pending(&mut self.state)
                .context("lost the Wayland connection")?
                > 0
            {
                continue;
            }

            let left = deadline.saturating_duration_since(Instant::now());
            anyhow::ensure!(!left.is_zero(), "the compositor took too long");

            let Some(guard) = self.queue.prepare_read() else {
                continue;
            };

            let timeout = Timespec {
                tv_sec: left.as_secs().try_into().unwrap_or(i64::MAX),
                tv_nsec: left.subsec_nanos().into(),
            };
            let mut polling = [PollFd::new(&self.connection, PollFlags::IN)];

            if rustix::event::poll(&mut polling, Some(&timeout)).unwrap_or(0) == 0 {
                anyhow::bail!("the compositor took too long");
            }

            let _ = guard.read();
        }

        Ok(())
    }
}

/// What the compositor has said about the capture in progress.
#[derive(Default)]
struct Frame {
    size: Option<(u32, u32)>,
    formats: Vec<wl_shm::Format>,
    described: bool,
    stopped: bool,
    ready: bool,
    failed: Option<String>,
}

impl Frame {
    /// The format to ask for: one without an alpha channel where there's a
    /// choice, since a window's thumbnail wants to be opaque.
    fn format(&self) -> Option<wl_shm::Format> {
        let opaque = self
            .formats
            .iter()
            .find(|format| matches!(format, wl_shm::Format::Xrgb8888));

        opaque
            .or_else(|| {
                self.formats
                    .iter()
                    .find(|format| matches!(format, wl_shm::Format::Argb8888))
            })
            .copied()
    }
}

#[derive(Default)]
struct State {
    shm: Option<wl_shm::WlShm>,
    sources: Option<ExtForeignToplevelImageCaptureSourceManagerV1>,
    captures: Option<ExtImageCopyCaptureManagerV1>,
    /// Every window the compositor lists, by the identifier it gives it.
    windows: HashMap<String, ExtForeignToplevelHandleV1>,
    frame: Frame,
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        queue: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name, interface, ..
        } = event
        else {
            return;
        };

        match interface.as_str() {
            "wl_shm" => state.shm = Some(registry.bind(name, 1, queue, ())),
            "ext_foreign_toplevel_list_v1" => {
                let _: ExtForeignToplevelListV1 = registry.bind(name, 1, queue, ());
            }
            "ext_foreign_toplevel_image_capture_source_manager_v1" => {
                state.sources = Some(registry.bind(name, 1, queue, ()));
            }
            "ext_image_copy_capture_manager_v1" => {
                state.captures = Some(registry.bind(name, 1, queue, ()));
            }
            _ => {}
        }
    }
}

impl Dispatch<ExtForeignToplevelListV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ExtForeignToplevelListV1,
        _: ext_foreign_toplevel_list_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }

    event_created_child!(State, ExtForeignToplevelListV1, [
        ext_foreign_toplevel_list_v1::EVT_TOPLEVEL_OPCODE => (ExtForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ExtForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        handle: &ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_foreign_toplevel_handle_v1::Event::Identifier { identifier } => {
                state.windows.insert(identifier, handle.clone());
            }
            ext_foreign_toplevel_handle_v1::Event::Closed => {
                state.windows.retain(|_, listed| listed != handle);
            }
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureSessionV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ExtImageCopyCaptureSessionV1,
        event: ext_image_copy_capture_session_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_image_copy_capture_session_v1::Event::BufferSize { width, height } => {
                state.frame.size = Some((width, height));
            }
            ext_image_copy_capture_session_v1::Event::ShmFormat { format } => {
                if let Ok(format) = format.into_result() {
                    state.frame.formats.push(format);
                }
            }
            ext_image_copy_capture_session_v1::Event::Done => state.frame.described = true,
            ext_image_copy_capture_session_v1::Event::Stopped => state.frame.stopped = true,
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureFrameV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ExtImageCopyCaptureFrameV1,
        event: ext_image_copy_capture_frame_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_image_copy_capture_frame_v1::Event::Ready => state.frame.ready = true,
            ext_image_copy_capture_frame_v1::Event::Failed { reason } => {
                state.frame.failed = Some(format!("{reason:?}"));
            }
            _ => {}
        }
    }
}

delegate_noop!(State: ignore wl_shm::WlShm);
delegate_noop!(State: ignore wl_shm_pool::WlShmPool);
delegate_noop!(State: ignore wl_buffer::WlBuffer);
delegate_noop!(State: ignore ExtForeignToplevelImageCaptureSourceManagerV1);
delegate_noop!(State: ignore ExtImageCaptureSourceV1);
delegate_noop!(State: ignore ExtImageCopyCaptureManagerV1);
