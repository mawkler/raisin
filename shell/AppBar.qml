// The applications: the open ones first, each with a dot per window under its
// icon and its key under that, then the ones with nothing open, faintly. A
// highlight slides to whichever one the switch is on.

pragma ComponentBehavior: Bound

import QtQuick

import "Theme.js" as Theme

Item {
    id: bar

    // The open applications by app id, in the order of their keys, and what
    // the daemon says about each.
    required property var openIds
    required property var rows
    // The applications with nothing open.
    required property var closed
    // The application the switch is on.
    required property string current
    required property bool icons
    required property bool animate
    required property string family
    required property string mono

    implicitWidth: cells.implicitWidth
    implicitHeight: cells.implicitHeight

    // An open application's cell, or nothing when it isn't one.
    function cellOf(appId) {
        openRepeater.count;
        const index = openIds.indexOf(appId);

        return index >= 0 ? openRepeater.itemAt(index) : null;
    }

    // The middle of an open application's cell.
    function centreOf(appId) {
        const cell = cellOf(appId);

        return cell ? cell.x + Theme.cellSize / 2 : width / 2;
    }

    // Behind the application being switched to.
    Rectangle {
        readonly property Item cell: bar.cellOf(bar.current)

        x: cell?.x ?? 0
        width: Theme.cellSize
        height: Theme.cellSize
        radius: Theme.cellRadius
        color: Theme.selection
        border.width: 1
        border.color: Theme.selectionEdge
        visible: cell !== null

        Behavior on x {
            enabled: bar.animate

            NumberAnimation {
                duration: Theme.duration
                easing.type: Easing.OutQuint
            }
        }
    }

    Row {
        id: cells

        spacing: Theme.cellGap

        Repeater {
            id: openRepeater

            model: bar.openIds

            Cell {
                required property string modelData
                readonly property var row: bar.rows[modelData] ?? null

                key: row?.key ?? ""
                icons: row?.icons ?? []
                name: row?.name ?? ""
                windows: row?.windows.length ?? 0
                showIcon: bar.icons
                family: bar.family
                mono: bar.mono
            }
        }

        // Between the applications that are open and the ones that aren't.
        Item {
            width: Theme.dotSize + 2 * Theme.dividerGap
            height: Theme.cellSize
            visible: bar.openIds.length > 0 && bar.closed.length > 0

            Rectangle {
                anchors.centerIn: parent
                width: Theme.dotSize
                height: width
                radius: width / 2
                color: Theme.muted
            }
        }

        Repeater {
            model: bar.closed

            Cell {
                required property var modelData

                key: modelData.key
                icons: modelData.icons
                name: modelData.name
                windows: 0
                showIcon: bar.icons
                family: bar.family
                mono: bar.mono
                opacity: Theme.closed
            }
        }
    }

    // One application.
    component Cell: Column {
        id: cell

        property string key
        property var icons: []
        property string name
        property int windows
        property bool showIcon: true
        property string family
        property string mono

        width: Theme.cellSize
        spacing: Theme.keyTop

        Item {
            width: Theme.cellSize
            height: Theme.cellSize

            AppIcon {
                id: icon

                anchors.centerIn: parent
                // Up a little, out of the way of the dots, when there are any.
                anchors.verticalCenterOffset: cell.windows > 0 ? -3 : 0
                width: Theme.iconSize
                height: width
                names: cell.showIcon ? cell.icons : []
                visible: path !== ""
            }

            // The initials of an application the icon theme has nothing for.
            Rectangle {
                anchors.fill: icon
                radius: 9
                color: Theme.monogramFill
                visible: !icon.visible

                Text {
                    anchors.centerIn: parent
                    text: cell.name.split(/\s+/).slice(0, 2).map(word => word.charAt(0)).join("").toUpperCase()
                    color: Theme.text
                    font.family: cell.family
                    font.pixelSize: 14
                    font.weight: Font.DemiBold
                }
            }

            // A dot per open window, the way a dock shows what is running.
            Row {
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.bottom: parent.bottom
                anchors.bottomMargin: 4
                spacing: Theme.dotGap

                Repeater {
                    model: Math.min(cell.windows, Theme.dots)

                    Rectangle {
                        width: Theme.dotSize
                        height: width
                        radius: width / 2
                        color: Theme.text
                    }
                }
            }
        }

        Keycap {
            anchors.horizontalCenter: parent.horizontalCenter
            text: cell.key
            family: cell.mono
            visible: cell.key !== ""
        }
    }
}
