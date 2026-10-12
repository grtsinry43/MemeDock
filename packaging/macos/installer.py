# The installer's window, read by dmgbuild, which writes it into the volume's .DS_Store
# without Finder's help: a 16:9 window whose content is background.svg rendered at 2x, the
# app on the left and an Applications shortcut on the right.
#
# Every number here is layout that the background's 20 point grid defines, so the two files
# are one design and must move together:
#
#   window_rect    640x460 -- 640x360 of content at 16:9, and 100 taller than that,
#                  because Finder keeps its own bars above and below the content.
#   icon_size      149 -- the size of the *slot*, not of the tile. The icns sits on Apple's
#                  grid, so macOS draws the tile at 824/1024 of its slot: 149 puts the tile
#                  at 120, six cells of the grid, so its edges land on the lines. A round
#                  128 would sit two points inside the lines on each side and look almost
#                  right without being right.
#   text_size      13 -- the label Finder draws under each icon, its own default.
#   icon_locations the two icons in the background's coordinate space, both on crossings
#                  of the grid at 160: the app at x 160, Applications at x 480.
#
# app and background arrive through -D; the volume name is positional. See make-dmg.sh.
app = defines["app"]
name = app.rsplit("/", 1)[-1]

format = "UDZO"
files = [app]
symlinks = {"Applications": "/Applications"}
background = defines["background"]
window_rect = ((200, 200), (640, 460))
default_view = "icon-view"
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
sidebar_width = 0
icon_size = 149
text_size = 13
icon_locations = {name: (160, 160), "Applications": (480, 160)}