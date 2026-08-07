//! Platform helpers (Win32) isolated from providers and UI code.
//!
//! Every Win32 API is wrapped in a safe `extern "system" fn` (per the
//! main.rs convention), so call sites need no `unsafe` blocks.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

/// Put a list of file paths on the clipboard as `CF_HDROP`, so pasting into
/// Explorer (or any shell view) copies/moves the actual files, not text.
pub fn copy_files_to_clipboard(paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Err("no paths to copy".into());
    }
    // Each path null-terminated, plus one extra terminator for the DROPFILES
    // list. DROPFILES.fFiles offset 20; we only need the pFiles payload, so
    // just build the packed null-terminated string list.
    let mut payload: Vec<u16> = Vec::new();
    for path in paths {
        payload.extend(OsStr::new(path).encode_wide());
        payload.push(0);
    }
    payload.push(0);

    let bytes = payload.len() * 2;
    // GMEM_MOVEABLE (0x0002) | GMEM_ZEROINIT (0x0040)
    let h_mem = unsafe { GlobalAlloc(0x0002 | 0x0040, bytes) };
    if h_mem == 0 {
        return Err("GlobalAlloc failed".into());
    }
    let ptr = unsafe { GlobalLock(h_mem) };
    if ptr.is_null() {
        unsafe { GlobalFree(h_mem) };
        return Err("GlobalLock failed".into());
    }
    unsafe {
        std::ptr::copy_nonoverlapping(payload.as_ptr(), ptr as *mut u16, payload.len());
        GlobalUnlock(h_mem);
    }
    let ok = unsafe {
        OpenClipboard(0) != 0
            && EmptyClipboard() != 0
            && SetClipboardData(CF_HDROP, h_mem) != 0
            && CloseClipboard() != 0
    };
    if !ok {
        // The clipboard owns h_mem only after a successful SetClipboardData.
        unsafe { GlobalFree(h_mem) };
        return Err("clipboard transfer failed".into());
    }
    Ok(())
}

/// Lock the workstation (Ctrl+Alt+Del style).
pub fn lock_workstation() -> Result<(), String> {
    if unsafe { LockWorkStation() } == 0 {
        return Err("LockWorkStation failed".into());
    }
    Ok(())
}

/// Put the machine to sleep (no hibernation).
pub fn suspend_system() -> Result<(), String> {
    if unsafe { SetSuspendState(0, 0, 0) } == 0 {
        return Err("SetSuspendState failed".into());
    }
    Ok(())
}

/// How a captured clipboard bitmap was stored on the clipboard. `Png` means
/// the bytes are already a PNG file (apps like Chrome/Edge put `image/png`
/// on the clipboard); `Dib`/`DibV5` mean a device-independent bitmap with a
/// `BITMAPINFOHEADER` (respectively `BITMAPV5HEADER`) prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardBitmapFormat {
    Dib,
    DibV5,
    Png,
}

/// Read the current clipboard image, if any, preferring `CF_DIB`, then
/// `CF_DIBV5`, then the registered `PNG` / `image/png` formats. Returns the
/// raw bytes and which format they came from. Empty when no image is present
/// or the clipboard is locked by another window.
pub fn clipboard_bitmap_bytes() -> Option<(Vec<u8>, ClipboardBitmapFormat)> {
    let png_fmt = unsafe { RegisterClipboardFormatW(PNG_WIDE.as_ptr()) };
    let png_mime = unsafe { RegisterClipboardFormatW(PNG_MIME_WIDE.as_ptr()) };
    if unsafe { OpenClipboard(0) } == 0 {
        return None;
    }
    let chosen: Option<(u32, ClipboardBitmapFormat)> = unsafe {
        if IsClipboardFormatAvailable(CF_DIB) != 0 {
            Some((CF_DIB, ClipboardBitmapFormat::Dib))
        } else if IsClipboardFormatAvailable(CF_DIBV5) != 0 {
            Some((CF_DIBV5, ClipboardBitmapFormat::DibV5))
        } else if png_fmt != 0 && IsClipboardFormatAvailable(png_fmt) != 0 {
            Some((png_fmt, ClipboardBitmapFormat::Png))
        } else if png_mime != 0 && IsClipboardFormatAvailable(png_mime) != 0 {
            Some((png_mime, ClipboardBitmapFormat::Png))
        } else {
            None
        }
    };
    let out = chosen.and_then(|(fmt, tag)| {
        let h = unsafe { GetClipboardData(fmt) };
        if h == 0 {
            return None;
        }
        let ptr = unsafe { GlobalLock(h) };
        if ptr.is_null() {
            return None;
        }
        let size = unsafe { GlobalSize(h) };
        let mut bytes = vec![0u8; size];
        unsafe {
            std::ptr::copy_nonoverlapping(ptr, bytes.as_mut_ptr(), size);
            GlobalUnlock(h);
        }
        Some((bytes, tag))
    });
    unsafe {
        CloseClipboard();
    }
    out
}

/// Put raw bitmap bytes (in the given clipboard format) on the clipboard as
/// `CF_DIB`, `CF_DIBV5` or the registered `PNG` format. Used to restore an
/// image entry from history.
pub fn set_clipboard_bitmap(bytes: &[u8], format: ClipboardBitmapFormat) -> Result<(), String> {
    if bytes.is_empty() {
        return Err("empty bitmap".into());
    }
    let h_mem = unsafe { GlobalAlloc(0x0002 | 0x0040, bytes.len()) };
    if h_mem == 0 {
        return Err("GlobalAlloc failed".into());
    }
    let ptr = unsafe { GlobalLock(h_mem) };
    if ptr.is_null() {
        unsafe { GlobalFree(h_mem) };
        return Err("GlobalLock failed".into());
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
        GlobalUnlock(h_mem);
    }
    let fmt = match format {
        ClipboardBitmapFormat::Dib => CF_DIB,
        ClipboardBitmapFormat::DibV5 => CF_DIBV5,
        ClipboardBitmapFormat::Png => unsafe { RegisterClipboardFormatW(PNG_WIDE.as_ptr()) },
    };
    let ok = unsafe {
        OpenClipboard(0) != 0
            && EmptyClipboard() != 0
            && SetClipboardData(fmt, h_mem) != 0
            && CloseClipboard() != 0
    };
    if !ok {
        // The clipboard owns h_mem only after a successful SetClipboardData.
        unsafe { GlobalFree(h_mem) };
        return Err("clipboard transfer failed".into());
    }
    Ok(())
}

/// Decode `BITMAPINFOHEADER`/`BITMAPV5HEADER`-prefixed pixels (24 or 32 bpp,
/// top-down or bottom-up, `BI_RGB` or `BI_BITFIELDS` 32bpp) into RGBA.
/// Returns `(rgba, width, height)`; `None` for unsupported layouts.
pub fn dib_to_rgba(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    if bytes.len() < 40 {
        return None;
    }
    let width = i32::from_le_bytes(bytes[4..8].try_into().ok()?);
    let height = i32::from_le_bytes(bytes[8..12].try_into().ok()?);
    let bpp = u16::from_le_bytes(bytes[14..16].try_into().ok()?);
    if width <= 0 || height == 0 {
        return None;
    }
    let top_down = height < 0;
    let w = width as u32;
    let h = height.unsigned_abs();
    let pixels = &bytes[40..];
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    match bpp {
        32 => {
            let stride = w as usize * 4;
            for row in 0..h as usize {
                let src_row = if top_down { row } else { h as usize - 1 - row };
                let start = src_row * stride;
                if start + stride > pixels.len() {
                    return None;
                }
                for i in 0..w as usize {
                    let p = start + i * 4;
                    let (b, g, r, a) = (pixels[p], pixels[p + 1], pixels[p + 2], pixels[p + 3]);
                    rgba.extend_from_slice(&[r, g, b, a]);
                }
            }
        }
        24 => {
            let stride = ((w as usize * 3) + 3) & !3;
            for row in 0..h as usize {
                let src_row = if top_down { row } else { h as usize - 1 - row };
                let start = src_row * stride;
                if start + stride > pixels.len() {
                    return None;
                }
                for i in 0..w as usize {
                    let p = start + i * 3;
                    rgba.extend_from_slice(&[pixels[p + 2], pixels[p + 1], pixels[p], 255]);
                }
            }
        }
        _ => return None,
    }
    Some((rgba, w, h))
}

/// Encode RGBA pixels as a top-down 32 bpp `BI_RGB` DIB (the most widely
/// accepted layout for `CF_DIB`).
pub fn rgba_to_dib(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let pixel_bytes = rgba.len() as u32;
    let mut dib = Vec::with_capacity(40 + pixel_bytes as usize);
    dib.extend_from_slice(&40u32.to_le_bytes()); // biSize
    dib.extend_from_slice(&(width as i32).to_le_bytes()); // biWidth
    dib.extend_from_slice(&(-(height as i32)).to_le_bytes()); // biHeight (top-down)
    dib.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    dib.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    dib.extend_from_slice(&0u32.to_le_bytes()); // biCompression = BI_RGB
    dib.extend_from_slice(&pixel_bytes.to_le_bytes()); // biSizeImage
    dib.extend_from_slice(&[0u8; 16]); // biClrUsed..biClrImportant
    for px in rgba.chunks_exact(4) {
        // RGBA → BGRA
        dib.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }
    dib
}

const CF_DIB: u32 = 8;
const CF_DIBV5: u32 = 17;
const CF_HDROP: u32 = 15;

const PNG_WIDE: [u16; 4] = ['P' as u16, 'N' as u16, 'G' as u16, 0];
const PNG_MIME_WIDE: [u16; 10] = [
    'i' as u16, 'm' as u16, 'a' as u16, 'g' as u16, 'e' as u16, '/' as u16, 'p' as u16, 'n' as u16,
    'g' as u16, 0,
];

// MSDN: OpenClipboard — https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-openclipboard
#[link(name = "user32")]
extern "system" {
    fn OpenClipboard(hWndNewOwner: isize) -> i32;
    fn EmptyClipboard() -> i32;
    fn CloseClipboard() -> i32;
    fn LockWorkStation() -> i32;
    fn IsClipboardFormatAvailable(format: u32) -> i32;
    fn GetClipboardData(uFormat: u32) -> isize;
    fn RegisterClipboardFormatW(lpszFormat: *const u16) -> u32;
}

// MSDN: GlobalAlloc — https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-globalalloc
#[link(name = "kernel32")]
extern "system" {
    fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> isize;
    fn GlobalLock(hMem: isize) -> *mut u8;
    fn GlobalUnlock(hMem: isize) -> i32;
    fn GlobalFree(hMem: isize) -> isize;
    fn GlobalSize(hMem: isize) -> usize;
    fn SetClipboardData(uFormat: u32, hMem: isize) -> isize;
}

// MSDN: SetSuspendState — https://learn.microsoft.com/en-us/windows/win32/api/powrprof/nf-powrprof-setsuspendstate
#[link(name = "powrprof")]
extern "system" {
    fn SetSuspendState(hibernate: i32, force: i32, disable_wake_events: i32) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_path_list_is_rejected() {
        assert!(copy_files_to_clipboard(&[]).is_err());
    }

    #[test]
    fn dib_round_trip_preserves_pixels() {
        let rgba = [
            255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 10, 20, 30, 40,
        ];
        let dib = rgba_to_dib(&rgba, 2, 2);
        let (out, w, h) = dib_to_rgba(&dib).unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(out, rgba);
    }

    #[test]
    fn dib_24bpp_bottom_up_decodes() {
        // 2x2, bottom-up (positive height): first row in the buffer is the
        // BOTTOM row. Pixel (x=0,y=1) = red, (1,1) = green; (0,0) = blue, (1,0) = white.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&40u32.to_le_bytes());
        bytes.extend_from_slice(&2i32.to_le_bytes());
        bytes.extend_from_slice(&2i32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&24u16.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 16]);
        // bottom row (y=1): blue, white (+ 2 pad bytes)
        bytes.extend_from_slice(&[255, 0, 0, 255, 255, 255, 0, 0]);
        // top row (y=0): red, green (+ 2 pad bytes)
        bytes.extend_from_slice(&[0, 0, 255, 0, 255, 0, 0, 0]);
        let (rgba, w, h) = dib_to_rgba(&bytes).unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(
            rgba,
            vec![
                255, 0, 0, 255, // y=0 red
                0, 255, 0, 255, // y=0 green
                0, 0, 255, 255, // y=1 blue
                255, 255, 255, 255 // y=1 white
            ]
        );
    }

    #[test]
    fn dib_rejects_unsupported_layouts() {
        // Truncated header
        assert!(dib_to_rgba(&[0u8; 20]).is_none());
        // 8 bpp paletted
        let mut bytes = vec![0u8; 40];
        bytes[4..8].copy_from_slice(&4i32.to_le_bytes());
        bytes[8..12].copy_from_slice(&4i32.to_le_bytes());
        bytes[12..14].copy_from_slice(&1u16.to_le_bytes());
        bytes[14..16].copy_from_slice(&8u16.to_le_bytes());
        assert!(dib_to_rgba(&bytes).is_none());
    }
}
