// Run with: node --test crates/lbm-agent/tests/plasma_wallpaper_references.mjs
// Exercises the exact shipped script against a read-only Plasma model; no D-Bus.
import {readFileSync} from "node:fs";
import {runInNewContext} from "node:vm";
import assert from "node:assert/strict";
import test from "node:test";
const script = readFileSync(new URL("../src/plasma_wallpaper_references.js", import.meta.url), "utf8");
function containment(path, active = "org.kde.image") {
    return {$: {plugin: "org.kde.plasma.folder", wallpaperplugin: active},
        Wallpaper: {"org.kde.image": {General: {$: {Image: path}}}}};
}
function fixture() {
    const saved = {1: containment("file:///active.png"), 2: containment("file:///inactive.png")};
    const live = [{wallpaperPlugin: "org.kde.image", currentConfigGroup: ["original"],
        configGroups: ["org.kde.image"], readConfig: () => "file:///unflushed.png"}];
    let reply;
    function wrapper(node) {
        return {node, groupList: Object.keys(node).filter(k => k !== "$"),
            readEntry: key => node.$?.[key]};
    }
    return {saved, live, run() {
        runInNewContext(script, {
            ConfigFile(parent, group) {
                if (typeof parent === "string") {
                    assert.equal(parent, "plasma-org.kde.plasma.desktop-appletsrc");
                    assert.equal(group, "Containments");
                    return wrapper(saved);
                }
                return wrapper(parent.node[group] ?? {});
            },
            desktops: () => live,
            print(value) { reply = JSON.parse(value); },
        });
        return reply;
    }};
}
test("retains inactive containments and live unflushed images", () => {
    const f = fixture();
    assert.deepEqual(f.run(), {version: 1, paths: ["file:///active.png", "file:///inactive.png", "file:///unflushed.png"]});
    assert.deepEqual(f.live[0].currentConfigGroup, ["original"]);
});
test("a colour desktop retains its inactive image settings", () => {
    const f = fixture();
    f.saved[2].$.wallpaperplugin = "org.kde.color";
    f.live[0].wallpaperPlugin = "org.kde.color";
    assert.equal(f.run().paths.length, 3);
});
test("panels do not become wallpaper references", () => {
    const f = fixture();
    f.saved[3] = {$: {plugin: "org.kde.panel"}};
    assert.equal(f.run().paths.length, 3);
});
test("unknown active, inactive and live plugins abort", () => {
    for (const change of [
        f => f.saved[2].$.wallpaperplugin = "org.kde.slideshow",
        f => f.saved[2].Wallpaper["third.party"] = {},
        f => f.live[0].wallpaperPlugin = "third.party",
        f => f.live[0].configGroups.push("org.kde.slideshow"),
    ]) {
        const f = fixture(); change(f);
        assert.throws(() => f.run(), /Unsupported wallpaper plugin/);
        assert.deepEqual(f.live[0].currentConfigGroup, ["original"]);
    }
});
test("missing image configuration aborts", () => {
    for (const change of [
        f => delete f.saved[2].Wallpaper["org.kde.image"],
        f => delete f.saved[2].Wallpaper["org.kde.image"].General.$.Image,
        f => f.live[0].readConfig = () => "",
    ]) {
        const f = fixture(); change(f);
        assert.throws(() => f.run(), /Missing/);
        assert.deepEqual(f.live[0].currentConfigGroup, ["original"]);
    }
});
test("an empty saved or live inventory aborts", () => {
    const f = fixture(); f.live.length = 0;
    assert.throws(() => f.run(), /Incomplete/);
    const g = fixture(); delete g.saved[1]; delete g.saved[2];
    assert.throws(() => g.run(), /No saved/);
});
test("a colour-only desktop can return no file references", () => {
    const f = fixture();
    for (const c of Object.values(f.saved)) { c.$.wallpaperplugin = "org.kde.color"; c.Wallpaper = {}; }
    f.live[0].wallpaperPlugin = "org.kde.color";
    f.live[0].readConfig = () => "";
    f.live[0].configGroups = ["org.kde.color"];
    assert.deepEqual(f.run(), {version: 1, paths: []});
});
