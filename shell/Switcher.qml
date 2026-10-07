// The panel: what the switch is for and the keys that steer it, then the
// applications, then the windows of the one being switched to.
//
// It is one size for as long as the switcher is open. Switching to another
// application moves the highlight along the applications, and its windows
// grow out from under it as the last one's go back; nothing changes size.

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
    // The font keys are written in, so that every key is as wide as every
    // other. The desktop's own monospaced font when it is installed, and
    // otherwise whatever the system calls monospace, which always is.
    readonly property string mono: {
        const wanted = settings.mono ?? "";

        return wanted !== "" && Qt.fontFamilies().includes(wanted) ? wanted : "monospace";
    }
    readonly property int contentWidth: resolve(settings.width, screenWidth)
    readonly property int maxHeight: resolve(settings.maxHeight, screenHeight)

    // Everything in the panel but the windows.
    readonly property real chrome: Theme.padTop + title.implicitHeight + Theme.headingGap + bar.implicitHeight + Theme.barGap + Theme.padBottom
    // The room a window's title takes under its picture.
    readonly property real titleSpace: Theme.titleTop + titles.height
    // How tall a window's picture is: as tall as configured, unless that
    // would make the panel taller than it may be.
    readonly property int tall: Math.max(48, Math.min(settings.previewHeight, maxHeight - chrome - 2 * Theme.cardPad - titleSpace))

    // The window is exactly as big as the panel and its shadow.
    readonly property int surfaceWidth: Math.min(screenWidth, panel.width + 2 * Theme.margin)
    readonly property int surfaceHeight: Math.min(screenHeight, panel.height + 2 * Theme.margin)

    // The open applications by the app id they are known by, and the order
    // they go in, which only changes when the applications do: a switch moves
    // things along rather than building them again.
    property var openIds: []
    property var rows: ({})
    property var closed: []

    onSceneChanged: {
        if (!scene)
            return;

        const ids = scene.rows.map(row => row.appId);

        if (!same(ids, openIds))
            openIds = ids;

        const byId = {};

        for (const row of scene.rows)
            byId[row.appId] = row;

        rows = byId;

        const keys = scene.absent.map(chip => chip.key + "\n" + chip.name);

        if (!same(keys, closed.map(chip => chip.key + "\n" + chip.name)))
            closed = scene.absent;
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
        id: titles

        font.family: switcher.family
        font.pixelSize: 12
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
        height: switcher.chrome + strip.height
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
                    mono: switcher.mono
                    visible: key !== ""
                }

                Hint {
                    key: switcher.scene?.cycleKey ? "Shift + " + switcher.scene.cycleKey : ""
                    says: "previous"
                    family: switcher.family
                    mono: switcher.mono
                    visible: key !== ""
                }

                Hint {
                    key: switcher.settings.cancelKey
                    says: "cancel"
                    family: switcher.family
                    mono: switcher.mono
                }
            }
        }

        AppBar {
            id: bar

            anchors.horizontalCenter: parent.horizontalCenter
            y: Theme.padTop + title.implicitHeight + Theme.headingGap
            openIds: switcher.openIds
            rows: switcher.rows
            closed: switcher.closed
            current: switcher.scene?.target ?? ""
            icons: switcher.settings.icons
            animate: switcher.animate
            family: switcher.family
            mono: switcher.mono
        }

        // The windows, an application's at a time. Every open application's
        // windows wait here, one on top of the other and all but one hidden,
        // so that switching never has to build them again.
        Item {
            id: strip

            x: Theme.padSide
            y: bar.y + bar.height + Theme.barGap
            width: switcher.contentWidth
            height: 2 * Theme.cardPad + switcher.tall + switcher.titleSpace
            clip: true

            Repeater {
                model: switcher.openIds

                WindowPage {
                    id: windowPage

                    required property string modelData

                    // How far in this application's windows are: all the way
                    // while the switch is on it, and gone otherwise. Hidden,
                    // not taken away, so that they are still captured.
                    property real shown: current ? 1 : 0

                    Behavior on shown {
                        enabled: switcher.animate

                        NumberAnimation {
                            duration: Theme.duration
                            easing.type: Easing.OutQuint
                        }
                    }

                    width: strip.width
                    height: strip.height
                    // Squared, so the windows going are nearly gone before
                    // the ones coming are much there, rather than the two
                    // showing through each other.
                    opacity: shown * shown
                    // They come out of the application's own icon and go back
                    // into it, so they move a little towards it rather than
                    // across the panel.
                    transform: Scale {
                        origin.x: bar.x + bar.centreOf(windowPage.modelData) - strip.x
                        origin.y: 0
                        xScale: Theme.pageGrow + (1 - Theme.pageGrow) * windowPage.shown
                        yScale: xScale
                    }

                    windows: switcher.rows[modelData]?.windows ?? []
                    icons: switcher.rows[modelData]?.icons ?? []
                    current: switcher.scene?.target === modelData
                    selected: switcher.selected
                    tall: switcher.tall
                    previews: switcher.settings.previews
                    capturing: switcher.capturing
                    animate: switcher.animate
                    family: switcher.family
                }
            }
        }
    }

    // One key and what it does.
    component Hint: Row {
        id: hint

        property string key
        property string says
        property string family
        property string mono

        spacing: 5

        Keycap {
            anchors.verticalCenter: parent.verticalCenter
            text: hint.key
            family: hint.mono
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
