// The icon of an application that was just started, shown for about as long
// as it takes to notice.
//
// Two motions laid over one another. Arriving, it drifts up and grows into
// itself while it fades in, slowing as it lands. Leaving, it goes on growing
// while the fade does the opposite, holding for a moment and then taking it
// all at once. The two pulling against each other is what makes it read as
// something opening rather than something being removed.

import QtQuick

Item {
    id: splash

    property bool playing: false
    property var icons: []
    // Milliseconds into the animation.
    property real elapsed: 0

    // Quick to arrive and slower to leave: what it says is that something has
    // started, which is worth a glance and no more.
    readonly property int appearing: 70
    readonly property int leaving: 250
    // How long after that the window comes down regardless.
    readonly property int slack: 100

    // How big the icon is at the moment it is most solid, how small it starts
    // and how large it ends, against that size.
    readonly property int size: 96
    readonly property real grow: 0.85
    readonly property real grown: 1.4
    // How solid it already is when it appears: what it is there to say is
    // that something happened just now, and a fade that begins at nothing
    // says it a moment late.
    readonly property real faint: 0.35
    // How far it drifts up as it appears.
    readonly property real lift: 10

    // Room for it at its largest, wherever it drifts.
    implicitWidth: Math.round(size * grown)
    implicitHeight: implicitWidth + 2 * lift

    readonly property real arrived: eased(elapsed / appearing)
    readonly property real gone: Theme.clamp((elapsed - appearing) / leaving, 0, 1)

    function play(names) {
        icons = names;
        elapsed = 0;
        playing = true;
        motion.restart();
        ending.restart();
    }

    function stop() {
        motion.stop();
        ending.stop();
        playing = false;
    }

    // Fast to begin with and slowing as it arrives.
    function eased(through) {
        const left = 1 - Theme.clamp(through, 0, 1);

        return 1 - left * left * left;
    }

    NumberAnimation {
        id: motion

        target: splash
        property: "elapsed"
        from: 0
        to: splash.appearing + splash.leaving
        duration: splash.appearing + splash.leaving
    }

    // What ends it, rather than the last frame: a window left on the screen
    // would sit above everything.
    Timer {
        id: ending

        interval: splash.appearing + splash.leaving + splash.slack
        onTriggered: splash.playing = false
    }

    // Drawn at the largest it gets and scaled down, so that it is sharp at
    // every size it passes through.
    AppIcon {
        anchors.centerIn: parent
        anchors.verticalCenterOffset: (1 - splash.arrived) * splash.lift
        width: Math.round(splash.size * splash.grown)
        height: width
        names: splash.icons
        visible: splash.playing
        scale: (splash.grow + (1 - splash.grow) * splash.arrived + (splash.grown - 1) * splash.eased(splash.gone)) / splash.grown
        opacity: (splash.faint + (1 - splash.faint) * splash.arrived) * (1 - splash.gone * splash.gone)
    }
}
