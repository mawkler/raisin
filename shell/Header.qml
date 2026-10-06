// An application's key, its icon and its name.

pragma ComponentBehavior: Bound

import QtQuick

import "Theme.js" as Theme

Row {
    id: header

    property string key
    property var icons: []
    property string name
    // How wide the name's column is. Without one the name takes only the room
    // it needs, which is what a line of applications side by side wants.
    property real nameWidth: -1
    property bool showIcon: true
    property string family
    property string mono

    spacing: 7

    Keycap {
        anchors.verticalCenter: parent.verticalCenter
        text: header.key
        family: header.mono
        visible: header.key !== ""
    }

    // An application the icon theme has nothing for still takes the room an
    // icon would have, or its name would sit a little to the left of
    // everyone else's.
    AppIcon {
        anchors.verticalCenter: parent.verticalCenter
        width: Theme.iconSize
        height: Theme.iconSize
        names: header.icons
        visible: header.showIcon
    }

    Text {
        anchors.verticalCenter: parent.verticalCenter
        width: header.nameWidth >= 0 ? header.nameWidth : implicitWidth
        text: header.name.toUpperCase()
        color: Theme.muted
        font.family: header.family
        font.pixelSize: 11
        font.weight: Font.Bold
        font.letterSpacing: 1
        elide: Text.ElideRight
    }
}
