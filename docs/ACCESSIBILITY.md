# Keyboard, Narrator and display scaling

Atlas uses the same grid, tree, menus and splitters that applications import from `ui/sdk/`. Open a folder of images in Atlas to show its project browser. Tab through the controls or press Shift+Tab to move back.

| Control | Keyboard interaction | Information exposed to assistive technology |
| --- | --- | --- |
| Project assets grid | Arrows move by cell. Home and End move within the row. Ctrl+Home and Ctrl+End jump to opposite corners. Page Up and Page Down move by a viewport. | Grid name, total row and column counts, column headers, selected row and active cell. Virtualized rows retain their logical indices. |
| Project folders tree | Up and Down move through visible items. Right expands or enters a folder. Left collapses or goes to its parent. Type the beginning of a name to find it. | Tree and item names, level, position among siblings, selection and expansion state. |
| Pane separator | Arrow keys adjust its size. Home and End choose its bounds. Enter toggles between the minimum and the previous size. | Accessible name, orientation, controlled pane and percentage. |
| Menu, dialog and text field | Enter or an arrow opens a menu. Escape returns to the trigger. Tab leaves the menu. A modal dialog restores focus to its opener when closed. | Expanded state, named menu and items, native dialog semantics and associated field labels. |

## Display scaling

Give `<rb-native-view>` its size in CSS pixels. The runtime uses WebView2’s rasterization scale and page zoom to convert the element’s edges to Direct3D pixel coordinates. An Atlas window moved across three monitors at 100% and 125% scaling kept all four edges of the texture preview within one device pixel of the HTML bounds.

Windows UI Automation exposed **Project folders**, **Project assets** and **Folder column width** by name in the packaged Atlas app. The keyboard checks also covered menu and dialog focus restoration, enlarged grid rows and a visible focus outline in forced colors. Use the walkthrough below to check spoken announcements with your Narrator settings.

## Check an application with Narrator

1. Open Atlas, choose a folder and start Narrator with Ctrl+Windows+Enter. Move focus to **Project folders**. If Narrator is intercepting arrow keys, toggle scan mode with Narrator+Space.
2. Expand **Project files**, move to a child, then return to the parent. Listen for the item name and whether a branch is expanded. Tab to **Folder column width** and press an arrow. Check the reported percentage.
3. Tab to **Project assets**, then move between columns and to the last row. Check the announced name and the row or column context. Open and dismiss the display-channel menu and check where focus returns.
4. In **Settings → Accessibility**, try a larger text size and a contrast theme. Check that the active cell remains visible, rows hold their text, and keyboard focus can be followed. Move and maximize the app on monitors with different display scaling. Check that the native texture and HTML controls still line up.

Windows UI Automation exposes the WebView2 accessibility tree to screen readers. Windows and Narrator versions, verbosity and scan mode affect the words a user hears. Label each grid, tree and splitter in the consuming app. Set `aria-controls` on the splitter to the ID of its primary pane. Size virtual rows with `--rb-row-height` or the inherited font size. [Grid keyboard pattern](https://www.w3.org/WAI/ARIA/apg/patterns/grid/) · [Tree view pattern](https://www.w3.org/WAI/ARIA/apg/patterns/treeview/) · [Window splitter pattern](https://www.w3.org/WAI/ARIA/apg/patterns/windowsplitter/)
