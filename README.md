# sctk backend for egui

This is a **very early** and **work in progress** repository working on a smithay client toolkit backend for egui. This will allow for egui applications to run inside a layer_shell surface.

> [!WARNING]
> Do not use this, it is somewhat working, but far from finished.

## todo

This is a likely incomplete list of things that need to be done:

- [x] keyboard input
- [x] mouse button input
- [ ] scroll support
- [ ] clipboard, copy/cut/paste
- [ ] fractional scaling
- [ ] multiple windows
- [ ] ime support
- [ ] touch input
- [ ] drag and drop
- [ ] touchpad gestures (pinch to zoom, etc)


The code is also in a really dirty state, it'll take some time to clean it up and find a good way to structure and abstract over things.