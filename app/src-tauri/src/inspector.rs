//! 隐藏窗口截图器：加载画板 → WKWebView takeSnapshot → PNG。
//! query_element 复用同一机制：在隐藏窗口里执行 elementFromPoint，
//! 结果通过事件 mlcanvas-js-result 回传（asset 页面不走应用命令 IPC，
//! 事件插件命令由 core:default 能力放行）。

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::commands;
use crate::state::AppState;

const LOAD_TIMEOUT: Duration = Duration::from_secs(5);
const RESULT_TIMEOUT: Duration = Duration::from_secs(3);
/// 页面 load 完成到首帧绘制的等待
const PAINT_DELAY: Duration = Duration::from_millis(150);

pub struct Inspector {
    app: AppHandle,
    counter: AtomicU64,
    /// query_element 的 eval 结果经事件 mlcanvas-js-result 回传到此通道
    js_result_tx: Mutex<Option<mpsc::Sender<String>>>,
}

impl Inspector {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            counter: AtomicU64::new(0),
            js_result_tx: Mutex::new(None),
        }
    }

    /// 读取画板声明的尺寸（parse_artboard_size，含缺省值）
    fn artboard_size(&self, file_name: &str) -> Result<(u32, u32), String> {
        let (path, _) = self.artboard_paths(file_name)?;
        let html = std::fs::read_to_string(&path)
            .map_err(|e| format!("读取画板失败: {e}"))?;
        Ok(commands::parse_artboard_size(&html))
    }

    /// 当前激活项目里某画板的（路径, 尺寸）
    fn artboard_paths(&self, file_name: &str) -> Result<(PathBuf, (u32, u32)), String> {
        let name = commands::validate_file_name(file_name)?;
        let dir = {
            let st = self.app.state::<AppState>();
            commands::artboards_dir(&st)
        };
        let path = dir.join(&name);
        if !path.is_file() {
            return Err(format!("画板不存在: {file_name}"));
        }
        let html = std::fs::read_to_string(&path).unwrap_or_default();
        Ok((path, commands::parse_artboard_size(&html)))
    }

    /// 打开隐藏窗口加载画板（窗口尺寸 = 画板尺寸），返回窗口和页面加载完成的信号。
    fn load_artboard(
        &self,
        file_name: &str,
    ) -> Result<(WebviewWindow, mpsc::Receiver<()>, (u32, u32)), String> {
        let (path, size) = self.artboard_paths(file_name)?;

        let url = asset_url(&path)?;
        let label = format!("mlcanvas-shot-{}", self.counter.fetch_add(1, Ordering::SeqCst));
        let (tx, rx) = mpsc::channel();
        let window = WebviewWindowBuilder::new(&self.app, &label, WebviewUrl::External(url))
            .title("MLCanvas Inspector")
            .visible(false)
            .inner_size(size.0 as f64, size.1 as f64)
            .on_page_load(move |_, payload| {
                if payload.event() == PageLoadEvent::Finished {
                    let _ = tx.send(());
                }
            })
            .build()
            .map_err(|e| format!("创建截图窗口失败: {e}"))?;
        Ok((window, rx, size))
    }

    fn wait_loaded(
        window: &WebviewWindow,
        rx: mpsc::Receiver<()>,
    ) -> Result<(), String> {
        rx.recv_timeout(LOAD_TIMEOUT)
            .map_err(|_| format!("画板加载超时: {}", window.label()))
    }

    pub fn screenshot(&self, file_name: &str) -> Result<Vec<u8>, String> {
        let (window, rx, size) = self.load_artboard(file_name)?;
        let outcome = (|| -> Result<Vec<u8>, String> {
            Self::wait_loaded(&window, rx)?;
            std::thread::sleep(PAINT_DELAY);

            let (tx, png_rx) = mpsc::channel::<Result<Vec<u8>, String>>();
            window
                .with_webview(move |platform| {
                    #[cfg(target_os = "macos")]
                    macos::request_snapshot(platform.inner(), size, tx);
                    #[cfg(not(target_os = "macos"))]
                    {
                        let _ = (tx, size);
                    }
                })
                .map_err(|e| format!("访问 webview 失败: {e}"))?;

            png_rx
                .recv_timeout(LOAD_TIMEOUT)
                .map_err(|_| "截图超时".to_string())?
        })();

        let _ = window.close();
        outcome
    }

    pub fn query_element(&self, file_name: &str, x: f64, y: f64) -> Result<Value, String> {
        let (window, rx, _) = self.load_artboard(file_name)?;
        let outcome = (|| -> Result<Value, String> {
            Self::wait_loaded(&window, rx)?;

            let (tx, result_rx) = mpsc::channel::<String>();
            *self
                .js_result_tx
                .lock()
                .expect("js_result_tx lock poisoned") = Some(tx);

            let script = format!(
                "(function(){{\
                    function result(v){{\
                        window.__TAURI_INTERNALS__.invoke('plugin:event|emit_to',{{\
                            target:{{kind:'Any'}},\
                            event:'mlcanvas-js-result',\
                            payload:v}});\
                    }}\
                    try{{\
                        var el=document.elementFromPoint({x},{y});\
                        if(!el){{result(null);return;}}\
                        var r=el.getBoundingClientRect();\
                        result({{\
                            tag:el.tagName.toLowerCase(),\
                            id:el.id||null,\
                            classes:Array.prototype.slice.call(el.classList),\
                            text:(el.textContent||'').trim().slice(0,100),\
                            dataId:el.getAttribute('data-id'),\
                            rect:{{x:r.x,y:r.y,width:r.width,height:r.height}}\
                        }});\
                    }}catch(e){{result({{error:String(e)}});}}\
                }})()",
                x = x,
                y = y
            );
            window.eval(&script).map_err(|e| format!("eval 失败: {e}"))?;

            let raw = match result_rx.recv_timeout(RESULT_TIMEOUT) {
                Ok(raw) => raw,
                Err(_) => {
                    let url = window
                        .url()
                        .map(|u| u.to_string())
                        .unwrap_or_else(|_| "<no url>".into());
                    return Err(format!("query_element 结果等待超时（诊断 url={url}）"));
                }
            };
            serde_json::from_str(&raw).map_err(|e| format!("解析结果失败: {e}"))
        })();

        let _ = window.close();
        outcome
    }

    /// 调试：截取应用现有窗口（main/design）的内容，返回 PNG 字节
    pub fn screenshot_window(&self, label: &str) -> Result<Vec<u8>, String> {
        let window = self
            .app
            .get_webview_window(label)
            .ok_or_else(|| format!("窗口不存在: {label}"))?;
        let size: (u32, u32) = window
            .inner_size()
            .map(|s| (s.width, s.height))
            .map_err(|e| format!("读取窗口尺寸失败: {e}"))?;

        let (tx, png_rx) = mpsc::channel::<Result<Vec<u8>, String>>();
        window
            .with_webview(move |platform| {
                #[cfg(target_os = "macos")]
                macos::request_snapshot(platform.inner(), size, tx);
                #[cfg(not(target_os = "macos"))]
                {
                    let _ = (tx, size);
                }
            })
            .map_err(|e| format!("访问 webview 失败: {e}"))?;

        png_rx
            .recv_timeout(LOAD_TIMEOUT)
            .map_err(|_| "截图超时".to_string())?
    }

    /// 调试：在应用现有窗口里执行 JS（脚本需调用 result(v) 回传），复用 js_result 事件通道
    pub fn eval_in_window(&self, label: &str, script: &str) -> Result<Value, String> {
        let window = self
            .app
            .get_webview_window(label)
            .ok_or_else(|| format!("窗口不存在: {label}"))?;

        let (tx, result_rx) = mpsc::channel::<String>();
        *self
            .js_result_tx
            .lock()
            .expect("js_result_tx lock poisoned") = Some(tx);
        window
            .eval(script)
            .map_err(|e| format!("eval 失败: {e}"))?;

        let raw = result_rx
            .recv_timeout(RESULT_TIMEOUT)
            .map_err(|_| "eval 结果等待超时".to_string())?;
        serde_json::from_str(&raw).map_err(|e| format!("解析结果失败: {e}"))
    }

    /// 事件监听线程把 mlcanvas-js-result 的 payload 转交进来
    pub fn push_js_result(&self, payload: String) {
        if let Some(tx) = self.js_result_tx.lock().expect("js_result_tx lock poisoned").as_ref() {
            let _ = tx.send(payload);
        }
    }
}

/// 本地文件路径 → asset 协议地址（macOS: asset://localhost/<percent-encoded path>）
fn asset_url(path: &Path) -> Result<tauri::Url, String> {
    let s = path.to_str().ok_or("画板路径不是合法 UTF-8")?;
    let mut encoded = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                encoded.push(b as char)
            }
            _ => encoded.push_str(&format!("%{b:02X}")),
        }
    }
    tauri::Url::parse(&format!("asset://localhost/{encoded}")).map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
mod macos {
    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::{AnyThread, MainThreadOnly};
    use objc2_app_kit::{
        NSBitmapImageFileType, NSBitmapImageRep, NSCompositingOperation, NSGraphicsContext, NSImage,
    };
    use objc2_foundation::{NSError, NSPoint, NSRect, NSSize};
    use objc2_web_kit::{WKSnapshotConfiguration, WKWebView};
    use std::sync::mpsc::Sender;

    /// 在主线程发起 takeSnapshot（裁剪到画板尺寸），完成后把 PNG 字节发回调用线程。
    pub fn request_snapshot(
        wk_ptr: *mut std::ffi::c_void,
        size: (u32, u32),
        tx: Sender<Result<Vec<u8>, String>>,
    ) {
        let wk: &WKWebView = unsafe { &*(wk_ptr as *const WKWebView) };
        let config = unsafe { WKSnapshotConfiguration::new(wk.mtm()) };
        unsafe {
            config.setRect(NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(size.0 as f64, size.1 as f64),
            ));
        }
        let handler = RcBlock::new(move |image: *mut NSImage, _error: *mut NSError| {
            if image.is_null() {
                let _ = tx.send(Err("takeSnapshot 返回空图像".to_string()));
                return;
            }
            let Some(image) = (unsafe { Retained::retain(image) }) else {
                let _ = tx.send(Err("takeSnapshot 图像保留失败".to_string()));
                return;
            };
            let _ = tx.send(unsafe { render_png(&image) });
        });
        unsafe {
            let _: () = objc2::msg_send![
                wk,
                takeSnapshotWithConfiguration: Some(&*config),
                completionHandler: &*handler
            ];
        }
    }

    /// NSImage → PNG（经 NSBitmapImageRep 重绘，避免依赖 CoreGraphics 类型转换）
    unsafe fn render_png(image: &NSImage) -> Result<Vec<u8>, String> {
        let size = image.size();
        let w = size.width.round() as isize;
        let h = size.height.round() as isize;
        if w <= 0 || h <= 0 {
            return Err("截图尺寸异常".to_string());
        }

        let color_space = objc2_foundation::NSString::from_str("NSDeviceRGBColorSpace");
        let rep = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            w,
            h,
            8,
            4,
            true,
            false,
            &color_space,
            0,
            0,
        )
        .ok_or("创建 NSBitmapImageRep 失败")?;

        let ctx = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)
            .ok_or("创建 NSGraphicsContext 失败")?;
        NSGraphicsContext::setCurrentContext(Some(&ctx));
        let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(size.width, size.height));
        image.drawInRect_fromRect_operation_fraction(
            rect,
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
            NSCompositingOperation::Copy,
            1.0,
        );
        NSGraphicsContext::setCurrentContext(None);

        let props = objc2_foundation::NSDictionary::<
            objc2_app_kit::NSBitmapImageRepPropertyKey,
            objc2::runtime::AnyObject,
        >::dictionary();
        let data = rep
            .representationUsingType_properties(NSBitmapImageFileType::PNG, &props)
            .ok_or("PNG 编码失败")?;
        Ok(data.to_vec())
    }
}
