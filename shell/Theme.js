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

// Top to bottom: the heading line, the applications, and the windows of the
// one being switched to.
var headingGap = 18;
var barGap = 16;

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
// The room either side of the dot between open applications and closed ones.
var dividerGap = 8;

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
// The light under the application being switched to.
var glowWidth = 240;
var glowHeight = 36;
var glowBlur = 64;
// How much of the next window shows past the highlighted one, when they
// don't all fit.
var peek = 40;

var panel = "#f715171c";
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
var selection = "#386b8cff";
var selectionEdge = "#a686a4ff";
var glow = "#3386a4ff";
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

function clamp(value, lowest, highest) {
    return Math.max(lowest, Math.min(highest, value));
}
