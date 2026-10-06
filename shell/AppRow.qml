// One application: its key, its icon and its name, then its windows —
// thumbnails while it is the one being switched to, a line of markers the
// rest of the time.

pragma ComponentBehavior: Bound

import QtQuick

import "Theme.js" as Theme

Item {
    id: appRow

    required property var row
    required property bool open
    required property string selected
    required property var settings
    required property bool capturing
    required property bool animate
    required property string family
    // The font keys are written in.
    required property string mono
    required property real nameWidth
    // How wide every row's name is, so that every row's windows start in the
    // same place.
    required property real headerWidth

    readonly property real headerImplicitWidth: header.implicitWidth

    // How far the row is from a line of markers to a row of thumbnails. Its
    // windows all move together, so they share this rather than each easing
    // on its own.
    property real openness: open ? 1 : 0

    Behavior on openness {
        enabled: appRow.animate

        NumberAnimation {
            duration: Theme.duration
            easing.type: Easing.OutQuint
        }
    }

    // The windows by id, and the order they go in, which only changes when the
    // windows do: a switch moves the tiles it has rather than building them
    // again.
    property var windowIds: []
    readonly property var windows: {
        const byId = {};

        for (const window of row?.windows ?? [])
            byId[window.id] = window;

        return byId;
    }

    onRowChanged: {
        const ids = (row?.windows ?? []).map(window => window.id);

        if (ids.length !== windowIds.length || ids.some((id, index) => id !== windowIds[index]))
            windowIds = ids;
    }

    implicitHeight: Math.max(header.implicitHeight, tiles.height) + 2 * Theme.rowPad

    Header {
        id: header

        anchors.verticalCenter: parent.verticalCenter
        key: appRow.row?.key ?? ""
        icons: appRow.row?.icons ?? []
        name: appRow.row?.name ?? ""
        nameWidth: appRow.nameWidth
        showIcon: appRow.settings.icons
        family: appRow.family
        mono: appRow.mono
    }

    // A row with more windows than fit keeps the highlighted one in view.
    Flickable {
        id: scroller

        x: appRow.headerWidth + Theme.headerGap
        y: Theme.rowPad
        width: appRow.width - x
        height: tiles.height
        contentWidth: tiles.width
        contentHeight: tiles.height
        interactive: false
        clip: true
        contentX: appRow.openness * Theme.lerp(appRow.fromScroll, appRow.scrollFor(appRow.heldTile), appRow.glide)

        Item {
            id: tiles

            width: strip.width
            height: strip.height

            // Behind the window a release of Super would land on.
            Rectangle {
                readonly property Item tile: appRow.heldTile

                x: tile ? Theme.lerp(appRow.fromX, tile.x, appRow.glide) : 0
                width: tile ? Theme.lerp(appRow.fromWidth, tile.width, appRow.glide) : 0
                height: tile ? tile.height : 0
                radius: Theme.tileRadius
                color: Theme.selection
                border.width: 1
                border.color: Theme.selectionEdge
                opacity: appRow.openness
                visible: tile !== null && opacity > 0
            }

            Row {
                id: strip

                spacing: Theme.lerp(Theme.markerGap, Theme.tileGap, appRow.openness)

                Repeater {
                    id: tileRepeater

                    model: appRow.windowIds

                    WindowTile {
                        required property string modelData

                        window: appRow.windows[modelData] ?? null
                        openness: appRow.openness
                        selected: appRow.open && appRow.selected === modelData
                        capturing: appRow.capturing
                        settings: appRow.settings
                        icons: appRow.row?.icons ?? []
                        animate: appRow.animate
                        family: appRow.family
                    }
                }
            }
        }
    }

    // The highlight glides from one window to the next. It glides from where
    // it was rather than from the tile it was on, so that a key pressed in the
    // middle of a glide turns it around instead of starting it over; and it
    // tracks its tiles rather than where they were, so that it stays on them
    // while they move.
    readonly property Item selectedTile: {
        tileRepeater.count;
        const index = windowIds.indexOf(selected);

        return index >= 0 ? tileRepeater.itemAt(index) : null;
    }
    // The tile the highlight is on: the selected one, or while the row folds
    // shut, the one it was on.
    property Item heldTile: null
    property real fromX: 0
    property real fromWidth: 0
    property real fromScroll: 0
    property real glide: 1

    function scrollFor(tile) {
        const overflow = tiles.width - scroller.width;

        if (!tile || overflow <= 0)
            return 0;

        return Theme.clamp(tile.x + tile.width / 2 - scroller.width / 2, 0, overflow);
    }

    onSelectedTileChanged: {
        const tile = selectedTile;

        if (!tile)
            return;

        // Only a row that is already open glides: one opening has its
        // highlight fade in where it belongs.
        if (heldTile && animate && openness > 0.5) {
            fromX = Theme.lerp(fromX, heldTile.x, glide);
            fromWidth = Theme.lerp(fromWidth, heldTile.width, glide);
            fromScroll = Theme.lerp(fromScroll, scrollFor(heldTile), glide);
            glide = 0;
            gliding.restart();
        } else {
            gliding.stop();
            glide = 1;
        }

        heldTile = tile;
    }

    NumberAnimation {
        id: gliding

        target: appRow
        property: "glide"
        from: 0
        to: 1
        duration: Theme.duration
        easing.type: Easing.OutQuint
    }
}
