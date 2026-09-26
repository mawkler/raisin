# Window previews

How the switcher shows what each window actually looks like, rather than only its title.

This started as a plan and is now mostly built: the standard capture protocols, on a thread of
their own, feeding thumbnails into the rows of the application being switched to. What is still
only planned is called out at the bottom. The survey and the protocol notes are kept because they
are the reasoning behind what was built.

## What it has to leave alone

The switcher's two hard timing rules come first, and previews are additive to both:

- **A fast tap must show nothing.** Capturing has to start when the overlay is revealed, not when
  the key is pressed. A tap that never reveals anything must never capture anything either.
- **Confirming must not wait for drawing.** A preview that is still arriving, or never arrives,
  must not delay or block a switch. The session state machine in `src/daemon/controller.rs` stays
  exactly as it is: previews are a view concern, and the overlay shows a placeholder until a
  texture turns up.

That means no capture may ever be awaited on the path from a key press to a focused window.

## How other switchers do it

| Approach | Who uses it | Why it does or doesn't fit |
| --- | --- | --- |
| Compositor-internal scene access | GNOME Shell, KDE's Present Windows, Hyprland's `hyprexpo` plugin | Direct access to each window's texture, so it's free and always correct — but only from inside the compositor. As a Hyprland plugin it would tie raisin to a C++ ABI that changes every release, and rules out every other compositor. |
| Portal ScreenCast (xdg-desktop-portal + PipeWire) | OBS window capture, most screen sharing | Built for sharing, not thumbnails: a consent dialog per source, a PipeWire dependency, and a session setup measured in hundreds of milliseconds. Wrong shape for something that has to appear in 90 ms. |
| `wlr-screencopy-unstable-v1`, cropped to the window | `grim`, `wayshot`, and the crop-the-screenshot trick | Captures an output, not a window. Only works for windows currently on screen, so most of a grouped list would come back blank, stale, or showing whatever is on top of them. |
| `hyprland-toplevel-export-v1` | `xdg-desktop-portal-hyprland`'s window sharing | Captures one window by address, whether or not it's visible. Exact and available today — and Hyprland-only. |
| `ext-image-capture-source-v1` + `ext-image-copy-capture-v1` | The standardised successor to both wlr-screencopy and toplevel-export | Same capability, specified rather than vendor-specific, with a capture source for a foreign toplevel. This is the one to build against. |

## What Hyprland 0.56.2 actually implements

Verified by reading the source rather than the wiki:

- `src/protocols/ImageCaptureSource.cpp` exposes **both** capture source managers, including
  `ext_foreign_toplevel_image_capture_source_manager_v1` (`CToplevelImageCaptureSourceProtocol`).
- `src/protocols/ImageCopyCapture.cpp` opens a capture session per window
  (`Screenshare::mgr()->newSession(client, window)`).
- `src/managers/screenshare/ScreenshareFrame.cpp::renderWindow()` renders the window into the
  capture buffer with `g_pHyprRenderer->renderWindow(...)` and no visibility check, so **a window
  on another workspace still captures**. That is the single most important property for a switcher,
  and the crop-a-screenshot approach can't provide it.
- `src/protocols/ToplevelExport.cpp` is still there, so older Hyprland remains reachable by the
  vendor protocol.

Elsewhere, sway and niri have landed `ext-image-copy-capture-v1` for **outputs and cursors** but not
yet for toplevels. So building against the standard protocol buys nothing today beyond Hyprland —
and everything tomorrow, at no cost to us, as each compositor lands the toplevel source.

## Identifying a window across protocols

The capture source is obtained from an `ext-foreign-toplevel-list-v1` handle, while everything else
raisin knows about a window comes from Hyprland's IPC. They can be matched exactly, with no title
guessing:

- `ext_foreign_toplevel_handle_v1.identifier` is `std::format("{:x}", window->m_stableID)`
  (`src/protocols/ForeignToplevel.cpp:67`).
- `hyprctl clients -j` reports the same value as `"stableId": "{:x}"` (`src/debug/HyprCtl.cpp:423`).

So `Window` gains an `identifier: String` alongside `id`, filled in by each compositor integration,
and the preview layer never has to know what a Hyprland address is. Compositors without a stable
identifier can leave it empty and get no previews, which is a supported state.

## Shape of the code

```
src/compositor/mod.rs        trait Previews { fn capture(&self, windows: &[Window], sink: Sink); fn cancel(&self); }
src/preview/mod.rs           the Wayland client: its own connection, on its own thread
src/preview/ext_capture.rs   ext-image-copy-capture + ext-foreign-toplevel-list   (preferred)
src/preview/hypr_export.rs   hyprland-toplevel-export-v1                          (older Hyprland)
src/daemon/overlay.rs        a GdkTexture per row, placeholder until one arrives
```

- **Connection.** A second Wayland connection on a dedicated thread, mirroring how the Hyprland
  event socket is already read: a thread, an `async-channel`, and a task on the GTK main loop.
  Sharing GDK's connection would save a socket and cost a `Backend::from_foreign_display` and a
  lifetime problem; a dmabuf or shm buffer crosses connections happily, so there's nothing to gain.
- **Crates.** `wayland-client` plus `wayland-protocols` 0.32 with the `staging` feature, which has
  `ext::image_copy_capture::v1`, `ext::image_capture_source::v1` and `ext::foreign_toplevel_list::v1`.
  The Hyprland fallback needs its XML vendored and run through `wayland-scanner` in a build script.
- **Into GTK.** `shm` buffers become a `gdk::MemoryTexture` — one CPU copy, trivial at thumbnail
  sizes. That is where to start. The protocol hands out buffers at the window's full size, though,
  so a 4K window is a 33 MB copy; once several windows are captured at once, switch to
  `gdk::DmabufTextureBuilder` (GTK 4.14+, `v4_14` on the gtk4 crate) and let the GPU do the
  scaling.
- **Where it hooks in.** `Effect::Fill` starts captures once the switcher is on screen, and
  `Effect::Retitle` re-orders them when the user swaps application, so the group now being looked
  at is captured first; ending a session cancels them. Never on `Effect::ScheduleReveal`, which is
  what keeps a fast tap free of work.
- **One frame, not a stream.** An `ext-image-copy-capture` session delivers frames continuously;
  raisin captures one per window per reveal and stops. Content that changes while the switcher is
  open is not worth a frame loop.

## What can go wrong

| Failure | What the user should see |
| --- | --- |
| Compositor has no toplevel capture source | Titles only, exactly as today. Not an error. |
| A client hasn't drawn since it was last visible | Whatever it last drew, which is what every other switcher shows too. |
| Capture takes longer than the switch | The row keeps its placeholder, the switch is unaffected. |
| XWayland windows | Expected to work through the same path; worth an explicit test, since XWayland surfaces have bitten screencopy implementations before. |
| A window positioned entirely off the monitor | Its application's icon instead. Hyprland copies a window only while its box overlaps the monitor being rendered (`CScreenshareManager`), and skips the request silently rather than refusing it, so the client can only time out. Windows on hidden workspaces usually still overlap by a sliver and do capture. |
| A huge window, or many at once | Captures run one at a time, the group being switched to first, and a newer request abandons whatever is still running. |

## What was built

`src/preview/capture.rs` is the Wayland client: it binds the three protocols above, keeps every
window the compositor lists by its identifier, and captures one into shared memory on demand.
`src/preview/mod.rs` runs it on a thread, takes requests over a channel and sends thumbnails back,
and scales each capture down — averaging the pixels that fall into each one, so a window full of
text doesn't come back as noise.

The daemon asks for thumbnails in `Effect::Fill`, which the state machine only emits once the
switcher is on screen. A tap quick enough to skip the overlay therefore captures nothing, and that
is a property of the controller's tests rather than of timing.

Thumbnails are bounded in both directions, so a portrait window doesn't make its row three times
the height of the others, and each is captured at the size it is shown at: a `GtkPicture` asks for
as much room as its texture is wide, so a larger capture would stretch the panel rather than sharpen
the picture.

Verified in a nested Hyprland with `grim`: three terminals captured into their rows, a fast tap that
switched in 4 ms with no overlay and no captures, and a confirm that focused its window 2 ms after
Super came up while captures were in flight.

## Still only planned

- **dmabuf.** Everything goes through shared memory and a CPU downscale today. On a 4K window that
  is a 33 MB read per capture, on a thread of its own, which has not been worth avoiding yet.
- **The fallback** to `hyprland-toplevel-export-v1` for Hyprland older than the standard protocols.
  The trait boundary is there; the second implementation is not.
- **Capturing the whole list** rather than the group being switched between, which would make
  swapping application instant at proportionally more cost.
- **XWayland**, which should work through the same path but has not been tried.

## Settled since

- **Shape.** Every thumbnail is the same height and as wide as its own window, so a row of them
  reads as the windows themselves. A tile is sized from the window's geometry before its capture
  arrives, which keeps the strip from reflowing as thumbnails land; a compositor that won't report
  geometry falls back to a landscape shape until the capture corrects it.
- **Layout.** Every window gets a thumbnail, not only the group being switched between: the strip
  runs sideways, so a thumbnail on every tile costs width rather than height.
- **Previews survive a session**, cached by identifier, so a switcher opened a second time shows
  windows rather than empty boxes. The open question was whether a stale thumbnail is worse than
  none; it isn't. The cache is keyed by window, so it can never show one window's contents under
  another's name, and a fresh capture replaces it as soon as one arrives — the same "whatever it
  last drew" the table above already accepts. The cache is cleared when the configured width
  changes, since a texture captured at the old size would widen its tile.
- **Retargeting doesn't rebuild.** Pointing the switch at another application changes the heading
  and the highlight only. The strip already holds every window, so there is nothing to rebuild,
  and the thumbnails stay where they are.

## Open questions

- Whether a strip of thumbnails above the list would beat one per tile, if the tiles start to feel
  wide.
