// An application's icon, under the first of its names that the icon theme
// has. The daemon knows what an application might call its icon; only this
// side has the theme to ask.

import QtQuick
import Quickshell
import Quickshell.Widgets

IconImage {
    id: icon

    property var names: []

    implicitSize: width
    mipmap: true
    source: {
        for (const name of icon.names ?? []) {
            const found = Quickshell.iconPath(name, true);

            if (found)
                return found;
        }

        return "";
    }
}
