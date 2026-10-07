// The sheet the windows of the application being switched to lie on, and the
// tab that rises from it behind that application in the bar: one shape, like
// a tab and its page, so that the windows plainly belong to it.

import QtQuick
import QtQuick.Shapes

import "Theme.js" as Theme

Shape {
    id: sheet

    // The sheet itself.
    required property real sheetX
    required property real sheetY
    required property real sheetWidth
    required property real sheetHeight
    // The tab: its left edge, how wide it is, and where its top is. With no
    // tab, it has no height and the sheet's top edge runs straight across.
    required property real tabX
    required property real tabWidth
    required property real tabTop
    required property bool tabbed

    readonly property real corner: Theme.sheetRadius
    readonly property real tabCorner: Theme.cellRadius
    readonly property real fillet: tabbed ? Theme.fillet : 0
    readonly property real tabTopEdge: tabbed ? tabTop : sheetY
    readonly property real rightEdge: sheetX + sheetWidth
    readonly property real bottomEdge: sheetY + sheetHeight
    // The tab's left edge, kept far enough from the sheet's corners for its
    // fillets to fit between them.
    readonly property real tabLeft: Theme.clamp(tabX, sheetX + corner + fillet, rightEdge - corner - fillet - tabWidth)

    preferredRendererType: Shape.CurveRenderer

    ShapePath {
        fillColor: Theme.sheetFill
        strokeColor: Theme.sheetEdge
        strokeWidth: 1

        startX: sheet.sheetX
        startY: sheet.sheetY + sheet.corner

        PathArc {
            x: sheet.sheetX + sheet.corner
            y: sheet.sheetY
            radiusX: sheet.corner
            radiusY: sheet.corner
        }

        // Along the top to the tab, and up into it round a fillet.
        PathLine {
            x: sheet.tabLeft - sheet.fillet
            y: sheet.sheetY
        }

        PathArc {
            x: sheet.tabLeft
            y: sheet.sheetY - sheet.fillet
            radiusX: sheet.fillet
            radiusY: sheet.fillet
            direction: PathArc.Counterclockwise
        }

        PathLine {
            x: sheet.tabLeft
            y: sheet.tabTopEdge + sheet.tabCorner
        }

        PathArc {
            x: sheet.tabLeft + sheet.tabCorner
            y: sheet.tabTopEdge
            radiusX: sheet.tabCorner
            radiusY: sheet.tabCorner
        }

        PathLine {
            x: sheet.tabLeft + sheet.tabWidth - sheet.tabCorner
            y: sheet.tabTopEdge
        }

        PathArc {
            x: sheet.tabLeft + sheet.tabWidth
            y: sheet.tabTopEdge + sheet.tabCorner
            radiusX: sheet.tabCorner
            radiusY: sheet.tabCorner
        }

        // Down the tab's other side, and out round its other fillet.
        PathLine {
            x: sheet.tabLeft + sheet.tabWidth
            y: sheet.sheetY - sheet.fillet
        }

        PathArc {
            x: sheet.tabLeft + sheet.tabWidth + sheet.fillet
            y: sheet.sheetY
            radiusX: sheet.fillet
            radiusY: sheet.fillet
            direction: PathArc.Counterclockwise
        }

        PathLine {
            x: sheet.rightEdge - sheet.corner
            y: sheet.sheetY
        }

        PathArc {
            x: sheet.rightEdge
            y: sheet.sheetY + sheet.corner
            radiusX: sheet.corner
            radiusY: sheet.corner
        }

        PathLine {
            x: sheet.rightEdge
            y: sheet.bottomEdge - sheet.corner
        }

        PathArc {
            x: sheet.rightEdge - sheet.corner
            y: sheet.bottomEdge
            radiusX: sheet.corner
            radiusY: sheet.corner
        }

        PathLine {
            x: sheet.sheetX + sheet.corner
            y: sheet.bottomEdge
        }

        PathArc {
            x: sheet.sheetX
            y: sheet.bottomEdge - sheet.corner
            radiusX: sheet.corner
            radiusY: sheet.corner
        }

        PathLine {
            x: sheet.sheetX
            y: sheet.sheetY + sheet.corner
        }
    }
}
