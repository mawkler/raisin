//@ pragma Env QSG_RENDER_LOOP=threaded
//@ pragma Env QT_WAYLAND_DISABLE_WINDOWDECORATION=1

// raisin's switcher, as its daemon describes it.
//
// The daemon decides everything and says so over a socket, one line of JSON
// at a time. This only draws what it is told, and moves smoothly between one
// thing it is told and the next.

pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Hyprland
import Quickshell.Wayland

ShellRoot {
    id: root

    // What the daemon last said.
    property var settings: ({
        width: { pixels: 900 },
        maxHeight: { portion: 0.4 },
        previewHeight: 105,
        previews: true,
        icons: true,
        cancelKey: "Esc",
        font: null
    })
    property var scene: null
    property string selected: ""
    property string subject: ""
    property bool shown: false

    // Whether changes move rather than land. Only while the switcher is on
    // screen: a switch is filled in before it is shown, and has to appear
    // already in place.
    property bool animate: false

    // Whether windows are being captured: from just after the switcher is
    // first drawn until it goes, so that its first frame never waits for a
    // capture to start.
    property bool capturing: false

    Connections {
        target: Quickshell

        // There is nobody looking at this as a configuration, so there is
        // nobody to tell that it reloaded.
        function onReloadCompleted() {
            Quickshell.inhibitReloadPopup();
        }

        function onReloadFailed(error) {
            Quickshell.inhibitReloadPopup();
        }
    }

    Socket {
        path: Quickshell.env("RAISIN_VIEW_SOCKET") || ""
        connected: path !== ""

        parser: SplitParser {
            onRead: line => root.receive(line)
        }
    }

    function receive(line) {
        let message;

        try {
            message = JSON.parse(line);
        } catch (error) {
            console.warn("raisin: unreadable message:", line);
            return;
        }

        switch (message.type) {
        case "config":
            root.settings = message;
            break;
        case "session":
            root.scene = message;
            root.selected = message.selected;
            root.subject = message.subject;
            break;
        case "select":
            root.selected = message.id;
            root.subject = message.subject;
            break;
        case "show":
            splash.stop();
            root.shown = true;
            root.animate = true;
            break;
        case "hide":
            root.animate = false;
            root.capturing = false;
            root.shown = false;
            break;
        case "starting":
            if (!root.shown)
                splash.play(message.icons);
            break;
        }
    }

    PanelWindow {
        id: window

        // The monitor being looked at, which is the one the switch starts
        // from.
        screen: {
            const focused = Hyprland.focusedMonitor?.name;
            const screens = Quickshell.screens;

            for (let i = 0; i < screens.length; i++) {
                if (screens[i].name === focused)
                    return screens[i];
            }

            return screens[0];
        }

        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.namespace: "raisin"
        // The switcher never takes the keyboard: it would become the focused
        // surface, and Hyprland would hand focus back to whatever had it when
        // the switcher went away — undoing the switch. Its keys arrive as
        // keybinds instead.
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
        exclusionMode: ExclusionMode.Ignore

        color: "transparent"
        // Nor does it take the pointer: everything goes through to whatever
        // is underneath.
        mask: Region {}

        // As big as the panel can ever get, and never any other size: the
        // panel moves inside it, rather than the compositor moving it about
        // every time the panel changes height.
        implicitWidth: switcher.surfaceWidth
        implicitHeight: switcher.surfaceHeight

        visible: root.shown || splash.playing

        Switcher {
            id: switcher

            anchors.fill: parent
            visible: root.shown

            settings: root.settings
            scene: root.scene
            selected: root.selected
            subject: root.subject
            capturing: root.capturing
            animate: root.animate
            screenWidth: window.screen?.width ?? 1920
            screenHeight: window.screen?.height ?? 1080
        }

        FrameAnimation {
            running: root.shown && !root.capturing
            onRunningChanged: frames = 0
            onTriggered: {
                if (++frames >= 2)
                    root.capturing = true;
            }

            property int frames: 0
        }

        Splash {
            id: splash

            anchors.fill: parent
        }
    }
}
