//! Asset source: gpui-kit's default icon set plus the extra Lucide icons the ribbon uses.
//! (The default bundle ships ~100 icons; anything else renders blank unless embedded here.)

use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

gpui_kit::assets::icon_assets!(
    pub RibbonIcons,
    [
        Save,
        FilePlus,
        FileDown,
        Printer,
        Scissors,
        ClipboardPaste,
        Type,
        ZoomIn,
        ZoomOut,
        Plug,
        Unplug,
        Usb,
        LayoutGrid,
        Grid3x3,
        Ruler,
        Rows3,
        RectangleVertical,
        RectangleHorizontal,
        Columns2,
        StretchHorizontal,
        Shuffle,
        PanelRight,
        LogOut,
        Upload,
        Import,
        Maximize,
        PencilLine,
        Download,
        House,
        ArrowLeft,
        ArrowDownRight,
        Trash,
        TextCursorInput,
        AArrowUp,
        AArrowDown,
        ChevronUp,
    ]
);

pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = RibbonIcons.load(path)? {
            return Ok(Some(bytes));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = gpui_kit::assets::Assets.list(path)?;
        paths.extend(RibbonIcons.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
