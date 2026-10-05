// One window: a thumbnail while its application is the one being switched to,
// a line-tall marker the rest of the time, and every shape in between while a
// switch moves from one to the other.
//
// A marker is a slice through the middle of its window, blurred and faint, so
// that it is recognisable by its colours while its title stays readable on
// top. Opening is the frame growing until it shows the whole window, as the
// blur clears.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Effects
import Quickshell.Hyprland
import Quickshell.Wayland
import Quickshell.Widgets

import "Theme.js" as Theme

Item {
    id: tile

    required property var window
    // 0 for a marker, 1 for a thumbnail.
    required property real openness
    required property bool selected
    required property bool capturing
    required property var settings
    // The application's icon, for a window that has no picture yet.
    required property var icons
    required property bool animate
    required property string family

    readonly property string label: window?.title ?? ""
    readonly property bool previews: settings.previews
    readonly property int tall: settings.previewHeight

    // Every thumbnail is the same height and as wide as its own window, so a
    // row of them reads as the windows themselves; and a marker is exactly as
    // wide as its thumbnail, so a row of markers has the same rhythm.
    readonly property real wide: Math.max(4, Math.round(tall * (window?.aspect ?? 1.6)))
    // A tall, narrow window would otherwise leave no room for its title.
    readonly property real roomy: Math.max(wide, tall)

    readonly property real pad: Theme.lerp(0, Theme.tilePad, openness)
    readonly property real frameWidth: Theme.lerp(wide, roomy, openness)
    readonly property real frameHeight: Theme.lerp(Theme.markerHeight, previews ? tall : 0, openness)

    width: frameWidth + 2 * pad
    height: frameHeight + openness * (Theme.titleTop + caption.implicitHeight) + 2 * pad

    // The window as Quickshell knows it, which is what gets captured.
    // Hyprland's own IPC writes an address with 0x in front, and Quickshell
    // without.
    readonly property var toplevel: {
        const address = (window?.id ?? "").replace(/^0x/, "");
        const toplevels = Hyprland.toplevels.values;

        for (let i = 0; i < toplevels.length; i++) {
            if (toplevels[i].address === address)
                return toplevels[i];
        }

        return null;
    }

    // How far its picture has come in: it fades up when the first frame
    // lands, rather than popping in over whatever stood in for it.
    property real arrived: capture.hasContent ? 1 : 0

    Behavior on arrived {
        enabled: tile.animate

        NumberAnimation {
            duration: Theme.duration
            easing.type: Easing.OutQuint
        }
    }

    // Whether this tile has started capturing its window. Not until the
    // switcher is on screen, so that setting a capture up never holds up the
    // switcher's first frame; and from then on for as long as the tile lasts,
    // so that it opens next time showing what its window last looked like.
    property bool started: false

    Component.onCompleted: started = capturing

    onCapturingChanged: {
        if (!capturing)
            return;

        // A capture that has shown something already has a context to take
        // a fresh picture with. A new one takes its first by itself.
        if (started && !capture.live && capture.hasContent)
            capture.captureFrame();

        started = true;
    }

    readonly property color markerFill: Theme.markerFill
    readonly property color thumbFill: Theme.thumbFill
    readonly property color markerEdge: Theme.markerEdge
    readonly property color thumbEdge: Theme.thumbEdge

    function mix(from, to, through) {
        return Qt.rgba(Theme.lerp(from.r, to.r, through), Theme.lerp(from.g, to.g, through), Theme.lerp(from.b, to.b, through), Theme.lerp(from.a, to.a, through));
    }

    ClippingRectangle {
        id: frame

        x: tile.pad
        y: tile.pad
        width: tile.frameWidth
        height: tile.frameHeight
        radius: Theme.lerp(Theme.markerRadius, Theme.thumbRadius, tile.openness)
        color: tile.mix(tile.markerFill, tile.thumbFill, tile.openness)
        border.width: 1
        border.color: tile.mix(tile.markerEdge, tile.thumbEdge, tile.openness)
        contentUnderBorder: true
        // Without pictures there is nothing for a frame to hold once the
        // window is a tile, which is then only its title.
        opacity: tile.previews ? 1 : 1 - tile.openness
        visible: opacity > 0

        // The whole window at the size of its thumbnail, centred: the frame
        // shows as much of it as it is tall.
        //
        // The blur goes on an item around the capture rather than on the
        // capture itself, which draws nothing at all into a layer of its own.
        Item {
            x: (parent.width - width) / 2
            y: (parent.height - height) / 2
            width: tile.wide
            height: tile.tall
            visible: tile.previews
            opacity: tile.arrived * Theme.lerp(Theme.markerShows, 1, tile.openness)

            layer.enabled: tile.openness < 1
            // Lifted a little as well as blurred: most windows are dark, and
            // a marker is there to show what colour they are.
            layer.effect: MultiEffect {
                blurEnabled: true
                blur: 1 - tile.openness
                blurMax: Theme.blur
                brightness: Theme.markerLift * (1 - tile.openness)
                saturation: Theme.markerColour * (1 - tile.openness)
            }

            ScreencopyView {
                id: capture

                anchors.fill: parent
                captureSource: tile.previews && tile.started ? (tile.toplevel?.wayland ?? null) : null
                // Live while it is a thumbnail, or becoming one. A marker is
                // too small and too blurred to be worth keeping up to date
                // while the switcher is open: it is taken again each time the
                // switcher appears instead.
                live: tile.capturing && tile.openness > 0
            }
        }

        // Some windows take a while to come back, or never do: the
        // application's own icon says which window the tile is, where an
        // empty frame says only that something is missing.
        AppIcon {
            anchors.centerIn: parent
            width: Math.round(tile.tall / 2)
            height: width
            names: tile.icons
            opacity: Theme.standin * (1 - tile.arrived) * tile.openness
            visible: tile.previews && opacity > 0
        }

        Text {
            x: Theme.markerInset
            width: parent.width - 2 * Theme.markerInset
            anchors.verticalCenter: parent.verticalCenter
            text: tile.label
            color: Theme.text
            font.family: tile.family
            font.pixelSize: 12
            elide: Text.ElideRight
            opacity: 1 - tile.openness
            visible: opacity > 0
        }
    }

    Text {
        id: caption

        x: tile.pad + 2
        y: tile.pad + tile.frameHeight + tile.openness * Theme.titleTop
        width: tile.frameWidth - 4
        text: tile.label
        color: tile.selected ? Theme.selectedText : Theme.text
        font.family: tile.family
        font.pixelSize: 12
        elide: Text.ElideRight
        opacity: tile.openness
        visible: opacity > 0

        Behavior on color {
            enabled: tile.animate

            ColorAnimation {
                duration: Theme.duration
            }
        }
    }
}
