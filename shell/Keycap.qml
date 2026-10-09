// A key, drawn as a key.

import QtQuick

Rectangle {
    id: keycap

    property alias text: label.text
    property string family

    implicitWidth: label.implicitWidth + 16
    implicitHeight: label.implicitHeight + 6
    radius: 7
    color: Theme.keycapFill
    border.width: 1
    border.color: Theme.keycapEdge

    Text {
        id: label

        anchors.centerIn: parent
        color: Theme.keycapText
        font.family: keycap.family
        font.pixelSize: 11
        font.weight: Font.DemiBold
    }
}
