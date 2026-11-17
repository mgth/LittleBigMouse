// Read only. Persisted containments include inactive activities and disconnected
// screens; live objects also cover settings not yet flushed to appletsrc.
(function () {
    var paths = [];
    function supported(plugin) {
        if (plugin !== "org.kde.image" && plugin !== "org.kde.color")
            throw new Error("Unsupported wallpaper plugin: " + plugin);
    }
    function image(value) {
        if (typeof value !== "string" || value.length === 0)
            throw new Error("Missing wallpaper image reference");
        paths.push(value);
    }
    var root = ConfigFile("plasma-org.kde.plasma.desktop-appletsrc", "Containments");
    var ids = root.groupList;
    if (!ids || ids.length === 0) throw new Error("No saved containments");
    var saved = 0;
    for (var i = 0; i < ids.length; ++i) {
        var c = ConfigFile(root, ids[i]);
        if (c.readEntry("plugin") === "org.kde.panel") continue;
        var active = c.readEntry("wallpaperplugin");
        supported(active);
        ++saved;
        var wallpapers = ConfigFile(c, "Wallpaper");
        var plugins = wallpapers.groupList;
        var hasImage = false;
        for (var j = 0; j < plugins.length; ++j) {
            supported(plugins[j]);
            if (plugins[j] === "org.kde.image") {
                hasImage = true;
                var general = ConfigFile(ConfigFile(wallpapers, plugins[j]), "General");
                image(general.readEntry("Image"));
            }
        }
        if (active === "org.kde.image" && !hasImage) throw new Error("Missing image group");
    }
    var live = desktops();
    if (!saved || !live || !live.length) throw new Error("Incomplete desktop inventory");
    for (var k = 0; k < live.length; ++k) {
        var d = live[k];
        supported(d.wallpaperPlugin);
        var previous = d.currentConfigGroup;
        try {
            // Also retain an inactive image configuration on a colour desktop.
            d.currentConfigGroup = ["Wallpaper"];
            var groups = d.configGroups;
            for (var n = 0; n < groups.length; ++n) supported(groups[n]);
            d.currentConfigGroup = ["Wallpaper", "org.kde.image", "General"];
            var value = d.readConfig("Image", "");
            if (d.wallpaperPlugin === "org.kde.image" || value !== "") image(value);
        } finally {
            d.currentConfigGroup = previous;
        }
    }
    print(JSON.stringify({version: 1, paths: paths}));
})();
