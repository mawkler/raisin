// The applications: the open ones first, each with a dot per window under its
// icon and its key under that, then the ones with nothing open, faintly. The
// one the switch is on sits in a tab, which the switcher draws behind the bar
// so that it can join the sheet below; this says where it is.

pragma ComponentBehavior: Bound

import QtQuick

Item {
    id: bar

    // The open applications by app id, in the order of their keys, and what
    // the daemon says about each.
    required property var openIds
    required property var rows
    // The applications with nothing open.
    required property var closed
    // The application the switch is on, and which of its windows.
    required property string current
    required property int selectedIndex
    required property bool icons
    required property bool animate
    required property string family
    required property string mono

    implicitWidth: cells.implicitWidth
    implicitHeight: cells.implicitHeight

    // Where the tab is: the left edge of the cell of the application being
    // switched to, gliding from one cell to the next.
    readonly property Item currentCell: cellOf(current)
    readonly property bool tabbed: currentCell !== null
    property real tabX: currentCell?.x ?? 0

    Behavior on tabX {
        enabled: bar.animate

        NumberAnimation {
            duration: Theme.duration
            easing.type: Easing.BezierSpline
            easing.bezierCurve: Theme.curve
        }
    }

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
                lit: modelData === bar.current ? Math.min(bar.selectedIndex, Theme.dots - 1) : -1
                showIcon: bar.icons
                family: bar.family
                mono: bar.mono
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
        // Which dot belongs to the window a switch would land on, when this
        // is the application it is on.
        property int lit: -1
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
                        required property int index

                        width: Theme.dotSize
                        height: width
                        radius: width / 2
                        color: index === cell.lit ? Theme.dotLit : Theme.dotDim
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
