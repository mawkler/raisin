.pragma library

// The switcher's look. Colours with an alpha are written #AARRGGBB.

// Room around the panel for its shadow. The window never changes size, so
// this is spent once rather than on every switch.
var margin = 72;

var panelRadius = 20;
var padTop = 18;
var padSide = 20;
var padBottom = 16;
var titleInset = 4;

// Top to bottom: the heading line, the applications, and the sheet holding
// the windows of the one being switched to.
var headingGap = 18;
var barGap = 16;

// The sheet, and the tab joining it to the application being switched to.
// It is the panel less this all round, with corners that nest inside the
// panel's.
var sheetInset = 12;
var sheetRadius = 10;
var sheetPadTop = 4;
var sheetPadBottom = 8;
// How far the tab reaches above its application's cell, and how round the
// corners are where it meets the sheet.
var tabRise = 6;
var fillet = 10;
// Between the windows and the keys that walk them.
var hintGap = 2;

// The applications: a cell each, with a dot per open window under the icon
// and the key that reaches it under the cell.
var cellSize = 54;
var cellRadius = 15;
var cellGap = 6;
var iconSize = 34;
var dotSize = 4;
var dotGap = 3;
// At most this many dots, however many windows there are.
var dots = 4;
var keyTop = 6;

// The windows of the application being switched to.
var cardRadius = 8;
// The room around a window, which the highlight fills.
var cardPad = 8;
var cardGap = 4;
var titleTop = 6;
var highlightRadius = 14;
// How big an application's windows are while they're away: they grow to full
// size out of its icon, and shrink back into it.
var pageGrow = 0.92;
// How much of the next window shows past the highlighted one, when they
// don't all fit.
var peek = 40;

// The panel is grey, and the tab and the sheet of windows hanging from it are
// set into it, darker, the way a selected tab is.
var panel = "#f720232b";
var panelEdge = "#14ffffff";
var shadow = "#8c000000";
var heading = "#eef1f7";
// The window a switch would land on, the hints, and a window's dots.
var muted = "#78819a";
var text = "#c4cad8";
var selectedText = "#ffffff";
var keycapText = "#b7bfd0";
var keycapEdge = "#1affffff";
var keycapFill = "#0fffffff";
// The window a switch would land on, and nothing else: the application is
// marked by the tab, so that the two never look like the same kind of thing.
var selection = "#386b8cff";
var selectionEdge = "#a686a4ff";
var sheetFill = "#ff121419";
var sheetEdge = "#0affffff";
// The dot of the window a switch would land on, and every other window's.
var dotLit = "#eef1f7";
var dotDim = "#5c6478";
var cardEdge = "#1affffff";
var cardFill = "#40000000";
// Initials, for an application the icon theme has nothing for.
var monogramFill = "#338f98ac";

// The key hints are worth a glance, and no more.
var faint = 0.4;
// An application with nothing open, which is there for its key.
var closed = 0.35;
// An icon standing in for a window that has no picture yet.
var standin = 0.35;

// Every movement takes this long, on the curve Hyprland moves its own
// windows and layers on.
var duration = 220;
// Except the highlight moving from one window to the next, which happens as
// fast as the key is pressed and has to keep up with it.
var cycle = 120;

function clamp(value, lowest, highest) {
    return Math.max(lowest, Math.min(highest, value));
}
