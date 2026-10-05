.pragma library

// The switcher's look, carried over from the GTK stylesheet it replaces.
// Colours with an alpha are written #AARRGGBB.

// Room around the panel for its shadow. The window never changes size, so
// this is spent once rather than on every switch.
var margin = 72;

var panelRadius = 18;
var padTop = 18;
var padSide = 16;
var padBottom = 14;
// Between the heading line and the applications, and either side of it.
var titleGap = 14;
var titleInset = 4;

// One application per row.
var rowGap = 10;
var rowPad = 2;
// Between an application's name and its windows.
var headerGap = 14;
// How many characters of an application's name its column has room for.
var nameChars = 14;
var iconSize = 16;

// A window of an application that isn't the one being switched to: a line
// tall, and as wide as its thumbnail would be.
var markerHeight = 24;
var markerRadius = 4;
var markerGap = 5;
var markerInset = 6;

// A window of the application being switched to.
var tileGap = 8;
var tilePad = 6;
var tileRadius = 10;
var thumbRadius = 6;
var titleTop = 6;

var panel = "#f715171c";
var panelEdge = "#14ffffff";
var shadow = "#8c000000";
var heading = "#eef1f7";
// The window a switch would land on, the applications' names and the hints.
var muted = "#78819a";
var text = "#c4cad8";
var selectedText = "#ffffff";
var keycapText = "#b7bfd0";
var keycapEdge = "#1affffff";
var keycapFill = "#0fffffff";
var selection = "#386b8cff";
var selectionEdge = "#a686a4ff";
// A thumbnail's frame is nearly black, which reads as a window showing
// something. A marker shows less, so it is a plain grey under its colours.
var thumbEdge = "#1affffff";
var thumbFill = "#40000000";
var markerEdge = "#24ffffff";
var markerFill = "#338f98ac";
var divider = "#17ffffff";

// The applications with nothing open and the key hints are worth a glance,
// and neither is worth the eye the windows themselves are worth.
var faint = 0.4;
// An icon standing in for a window that has no picture yet.
var standin = 0.35;
// How strongly a window shows through its marker: enough to recognise it by,
// and little enough for the marker's title to stay readable on top of it.
var markerShows = 0.45;
// How far a marker's window is blurred, at most.
var blur = 40;
// And how much it is brightened and saturated, so that its colours read.
var markerLift = 0.15;
var markerColour = 0.5;

// Every movement takes this long, on the curve Hyprland moves its own
// windows and layers on.
var duration = 180;

function lerp(from, to, through) {
    return from + (to - from) * through;
}

function clamp(value, lowest, highest) {
    return Math.max(lowest, Math.min(highest, value));
}
