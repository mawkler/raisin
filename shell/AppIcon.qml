// An application's icon, under the first of its names that the icon theme
// has. The daemon knows what an application might call its icon; only this
// side has the theme to ask.

import QtQuick
import Quickshell
import Quickshell.Widgets

IconImage {
    id: icon

    property var names: []
    // Where the icon was found, or nothing when the theme has none of its
    // names.
    readonly property string path: {
        for (const name of icon.names ?? []) {
            const found = Quickshell.iconPath(name, true);

            if (found)
                return found;
        }

        return "";
    }

    implicitSize: width
    mipmap: true
    source: path
}
