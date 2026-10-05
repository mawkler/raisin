// The panel: what the switch is for and the keys that steer it, then every
// application's windows, then the applications with nothing open.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Effects
import QtQuick.Layouts

import "Theme.js" as Theme

Item {
    id: switcher

    required property var settings
    required property var scene
    required property string selected
    required property string subject
    required property bool capturing
    required property bool animate
    required property int screenWidth
    required property int screenHeight

    readonly property string family: settings.font ?? ""
    readonly property int contentWidth: resolve(settings.width, screenWidth)
    readonly property int maxContentHeight: resolve(settings.maxHeight, screenHeight)
    readonly property int chromeHeight: Theme.padTop + title.implicitHeight + Theme.titleGap + Theme.padBottom

    // The window is as big as the panel can ever get, with room around it
    // for the shadow, so that it never has to change size.
    readonly property int surfaceWidth: Math.min(screenWidth, contentWidth + 2 * Theme.padSide + 2 * Theme.margin)
    readonly property int surfaceHeight: Math.min(screenHeight, chromeHeight + maxContentHeight + 2 * Theme.margin)

    // Every row's name takes the same width, so the windows all start in the
    // same place however long the applications are called. Measured the way
    // GTK measures a label's width in characters.
    readonly property real nameWidth: Math.ceil(Math.max(names.averageCharacterWidth, names.advanceWidth("0")) * Theme.nameChars)
    readonly property real headerWidth: {
        let widest = 0;

        for (let i = 0; i < rowRepeater.count; i++) {
            const row = rowRepeater.itemAt(i);

            if (row)
                widest = Math.max(widest, row.headerImplicitWidth);
        }

        return widest;
    }

    // The rows by the application they are for, and the order they go in.
    // The order only changes when the applications do, so a switch keeps
    // every row it had and moves it, rather than building them all again.
    property var rowIds: []
    property var rows: ({})
    property var absent: []

    onSceneChanged: {
        if (!scene)
            return;

        const ids = scene.rows.map(row => row.appId);

        if (!same(ids, rowIds))
            rowIds = ids;

        const byId = {};

        for (const row of scene.rows)
            byId[row.appId] = row;

        rows = byId;

        const chips = scene.absent.map(chip => chip.key + "\n" + chip.name);

        if (!same(chips, absent.map(chip => chip.key + "\n" + chip.name)))
            absent = scene.absent;
    }

    function same(these, those) {
        if (these.length !== those.length)
            return false;

        for (let i = 0; i < these.length; i++) {
            if (these[i] !== those[i])
                return false;
        }

        return true;
    }

    function resolve(length, screen) {
        if (!length)
            return 0;

        if (length.pixels !== undefined)
            return length.pixels;

        return Math.round(screen * length.portion);
    }

    FontMetrics {
        id: names

        font.family: switcher.family
        font.pixelSize: 11
        font.weight: Font.Bold
    }

    RectangularShadow {
        anchors.fill: panel
        offset.y: 20
        blur: 50
        radius: panel.radius
        color: Theme.shadow
    }

    Rectangle {
        id: panel

        anchors.centerIn: parent
        width: switcher.contentWidth + 2 * Theme.padSide
        height: Theme.padTop + title.implicitHeight + Theme.titleGap + strip.height + Theme.padBottom
        radius: Theme.panelRadius
        color: Theme.panel
        border.width: 1
        border.color: Theme.panelEdge

        // What the switch is for, the window it would land on, and the keys
        // that steer it — which are nearly the same every time, so they sit
        // at the end of the line where the eye passes over them.
        RowLayout {
            id: title

            x: Theme.padSide + Theme.titleInset
            y: Theme.padTop
            width: switcher.contentWidth - 2 * Theme.titleInset
            spacing: 8

            Text {
                text: "Switch to " + (switcher.scene?.name ?? "")
                color: Theme.heading
                font.family: switcher.family
                font.pixelSize: 15
                font.weight: Font.DemiBold
            }

            Text {
                Layout.fillWidth: true
                Layout.preferredWidth: 0
                text: switcher.subject
                color: Theme.muted
                font.family: switcher.family
                font.pixelSize: 13
                elide: Text.ElideRight
            }

            Row {
                leftPadding: 6
                spacing: 12
                opacity: Theme.faint

                // The application's own key walks its windows, and Shift
                // with it walks them the other way.
                Hint {
                    key: switcher.scene?.cycleKey ?? ""
                    says: "next"
                    family: switcher.family
                    visible: key !== ""
                }

                Hint {
                    key: switcher.scene?.cycleKey ? "Shift + " + switcher.scene.cycleKey : ""
                    says: "previous"
                    family: switcher.family
                    visible: key !== ""
                }

                Hint {
                    key: switcher.settings.cancelKey
                    says: "cancel"
                    family: switcher.family
                }
            }
        }

        Flickable {
            id: strip

            readonly property real overflow: column.implicitHeight - height

            x: Theme.padSide
            y: Theme.padTop + title.implicitHeight + Theme.titleGap
            width: switcher.contentWidth
            height: Math.min(column.implicitHeight, switcher.maxContentHeight)
            contentWidth: width
            contentHeight: column.implicitHeight
            interactive: false
            clip: overflow > 0
            contentY: Theme.lerp(switcher.fromY, switcher.scrollFor(switcher.targetRow), switcher.glide)

            Column {
                id: column

                width: strip.width
                spacing: Theme.rowGap

                Repeater {
                    id: rowRepeater

                    model: switcher.rowIds

                    AppRow {
                        required property string modelData

                        width: column.width
                        row: switcher.rows[modelData] ?? null
                        open: switcher.scene?.target === modelData
                        selected: switcher.selected
                        settings: switcher.settings
                        capturing: switcher.capturing
                        animate: switcher.animate
                        family: switcher.family
                        nameWidth: switcher.nameWidth
                        headerWidth: switcher.headerWidth
                    }
                }

                // A line to say that what is below it is a different kind of
                // thing: keys for what could be opened, rather than windows
                // that are.
                Item {
                    width: column.width
                    height: 13
                    visible: switcher.rowIds.length > 0 && switcher.absent.length > 0

                    Rectangle {
                        y: 10
                        width: parent.width
                        height: 1
                        color: Theme.divider
                    }
                }

                // Applications with nothing open share one line between them:
                // they are there so that their keys can be seen, which takes
                // a name and no more. Whatever doesn't fit is cut off.
                Item {
                    width: column.width
                    height: chips.implicitHeight + 2 * Theme.rowPad
                    visible: switcher.absent.length > 0
                    clip: true
                    opacity: Theme.faint

                    Row {
                        id: chips

                        y: Theme.rowPad
                        spacing: 14

                        Repeater {
                            model: switcher.absent

                            Header {
                                required property var modelData

                                key: modelData.key
                                icons: modelData.icons
                                name: modelData.name
                                showIcon: switcher.settings.icons
                                family: switcher.family
                            }
                        }
                    }
                }
            }
        }
    }

    // When there are more rows than fit, the strip keeps the one being
    // switched to in view, gliding from wherever it was when the switch
    // moved to another application.
    readonly property Item targetRow: {
        rowRepeater.count;
        const index = rowIds.indexOf(scene?.target ?? "");

        return index >= 0 ? rowRepeater.itemAt(index) : null;
    }
    property Item heldRow: null
    property real fromY: 0
    property real glide: 1

    function scrollFor(row) {
        if (!row || strip.overflow <= 0)
            return 0;

        return Theme.clamp(row.y + row.height / 2 - strip.height / 2, 0, strip.overflow);
    }

    onTargetRowChanged: {
        if (heldRow && targetRow && animate) {
            fromY = Theme.lerp(fromY, scrollFor(heldRow), glide);
            glide = 0;
            gliding.restart();
        } else {
            gliding.stop();
            glide = 1;
        }

        heldRow = targetRow;
    }

    NumberAnimation {
        id: gliding

        target: switcher
        property: "glide"
        from: 0
        to: 1
        duration: Theme.duration
        easing.type: Easing.OutQuint
    }

    // One key and what it does.
    component Hint: Row {
        id: hint

        property string key
        property string says
        property string family

        spacing: 5

        Keycap {
            anchors.verticalCenter: parent.verticalCenter
            text: hint.key
            family: hint.family
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: hint.says
            color: Theme.muted
            font.family: hint.family
            font.pixelSize: 11
        }
    }
}
