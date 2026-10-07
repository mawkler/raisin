// One application's windows, side by side: centred while they fit, and kept
// in view by their highlight when they don't.

pragma ComponentBehavior: Bound

import QtQuick

import "Theme.js" as Theme

Item {
    id: page

    required property var windows
    required property var icons
    // Whether this is the application the switch is on.
    required property bool current
    required property string selected
    required property int tall
    required property bool previews
    required property bool capturing
    required property bool animate
    required property string family

    // The windows by id, and the order they go in, which only changes when
    // the windows do: a switch moves the highlight over the cards it has
    // rather than building them again.
    property var windowIds: []
    readonly property var byId: {
        const byId = {};

        for (const window of windows)
            byId[window.id] = window;

        return byId;
    }

    onWindowsChanged: {
        const ids = windows.map(window => window.id);

        if (ids.length !== windowIds.length || ids.some((id, index) => id !== windowIds[index]))
            windowIds = ids;
    }

    function cardWidth(window) {
        const wide = Math.max(4, Math.round(tall * (window?.aspect ?? 1.6)));

        return Math.max(wide, tall) + 2 * Theme.cardPad;
    }

    // How wide the windows are side by side, to centre them when they fit.
    readonly property real natural: {
        let width = 0;

        for (const id of windowIds)
            width += cardWidth(byId[id]);

        return width + Math.max(0, windowIds.length - 1) * Theme.cardGap;
    }

    // The highlight goes where the switch is, while this is the application
    // it is on, and stays where it was otherwise.
    function follow() {
        const index = windowIds.indexOf(selected);

        if (current && index >= 0)
            list.currentIndex = index;
    }

    onSelectedChanged: follow()
    onCurrentChanged: follow()
    onWindowIdsChanged: follow()

    ListView {
        id: list

        anchors.horizontalCenter: parent.horizontalCenter
        width: Math.min(page.natural, page.width)
        height: page.height
        orientation: ListView.Horizontal
        interactive: false
        spacing: Theme.cardGap
        // Every card is made at once rather than as it scrolls into view, so
        // that each has its picture before it gets there.
        cacheBuffer: 10000
        model: page.windowIds

        highlightFollowsCurrentItem: true
        highlightMoveDuration: page.animate ? Theme.duration : 0
        highlightMoveVelocity: -1
        highlightResizeDuration: page.animate ? Theme.duration : 0
        highlightResizeVelocity: -1
        // Windows that don't fit scroll just far enough to keep the
        // highlighted one whole, with a little of the next one showing, so
        // that it is plain there is more.
        highlightRangeMode: ListView.ApplyRange
        preferredHighlightBegin: Theme.peek
        preferredHighlightEnd: width - Theme.peek

        highlight: Rectangle {
            radius: Theme.highlightRadius
            color: Theme.selection
            border.width: 1
            border.color: Theme.selectionEdge
            opacity: page.current ? 1 : 0

            Behavior on opacity {
                enabled: page.animate

                NumberAnimation {
                    duration: Theme.duration
                    easing.type: Easing.OutQuint
                }
            }
        }

        delegate: WindowCard {
            required property string modelData
            required property int index

            window: page.byId[modelData] ?? null
            icons: page.icons
            selected: page.current && ListView.isCurrentItem
            current: page.current
            tall: page.tall
            previews: page.previews
            capturing: page.capturing
            animate: page.animate
            family: page.family
        }
    }
}
