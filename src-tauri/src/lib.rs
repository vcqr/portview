#[cfg(target_os = "windows")]
use image;
use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState};
use serde::{Deserialize, Serialize};
use sysinfo::{
    Networks, Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind, Users,
};
use tauri::{Emitter, Manager};

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Instant, SystemTime};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpConnection {
    pub protocol: String,
    pub local_addr: String,
    pub local_port: u16,
    pub remote_addr: String,
    pub remote_port: u16,
    pub state: String,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
    pub exe_path: Option<String>, // 可执行文件路径，前端据此从快照的 icons 映射中取图标
    pub start_time: Option<u64>,  // Process start time in seconds since Unix epoch
}

// get_connections 的返回结构：图标按唯一的 exe_path 去重后单独传输，
// 避免同一份 base64 图标随每条连接行重复序列化
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionsSnapshot {
    pub connections: Vec<TcpConnection>,
    pub icons: HashMap<String, String>,
    pub net_rate: NetRate,
}

// 系统网络吞吐：速率（字节/秒）与会话累计总量（应用启动以来）
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct NetRate {
    pub down_bps: u64,
    pub up_bps: u64,
    pub total_down: u64,
    pub total_up: u64,
}

// 定义进程详情结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessDetails {
    pub pid: u32,
    pub name: String,
    pub command_line: String,
    pub executable_path: String,
    pub memory_usage: u64,
    pub cpu_usage: f32,
    pub parent_pid: Option<u32>,
    pub start_time: u64,
}

// 调用链节点：祖先链与子进程列表共用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageNode {
    pub pid: u32,
    pub ppid: Option<u32>,
    pub name: String,
    pub command_line: String,
    pub exe_path: Option<String>,
    pub start_time: Option<u64>,
    pub user: Option<String>,
    pub icon: Option<String>, // Base64 PNG，按 exe_path 复用全局图标缓存
    // 该节点的父进程已退出（PPID 槽位被回收复用或父进程已消失），
    // 祖先链在此断裂——语义对齐 witr 的 ParentExited
    pub parent_exited: bool,
    pub is_target: bool, // 是否为当前查看的目标进程
}

// 进程调用链：沿 PPID 向上追溯到 PID 1 的完整祖先链 + 直接子进程
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessLineage {
    pub nodes: Vec<LineageNode>,    // root → target 顺序
    pub children: Vec<LineageNode>, // 目标进程的直接子进程
}

// 端口绑定条目：菜单栏"追踪"入口按端口实时解析的结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortBinding {
    pub protocol: String,           // TCP / UDP
    pub local_addr: String,
    pub state: String,              // LISTEN / ESTABLISHED / UNCONN ...
    pub pid: Option<u32>,
    pub process_name: Option<String>,
}

// 将 netstat2 的 TCP 状态转换为字符串
fn tcp_state_to_string(state: TcpState) -> &'static str {
    match state {
        TcpState::Established => "ESTABLISHED",
        TcpState::SynSent => "SYN_SENT",
        TcpState::SynReceived => "SYN_RECV",
        TcpState::FinWait1 => "FIN_WAIT1",
        TcpState::FinWait2 => "FIN_WAIT2",
        TcpState::TimeWait => "TIME_WAIT",
        TcpState::Closed => "CLOSED",
        TcpState::CloseWait => "CLOSE_WAIT",
        TcpState::LastAck => "LAST_ACK",
        TcpState::Listen => "LISTEN",
        TcpState::Closing => "CLOSING",
        TcpState::DeleteTcb => "DELETE_TCB",
        TcpState::Unknown => "UNKNOWN",
    }
}

// ==================== 跨平台缓存机制 ====================

// 全局缓存目录路径
lazy_static::lazy_static! {
    static ref CACHE_DIR_PATH: Option<PathBuf> = initialize_cache_directory();
}

// 全局偏好：关闭窗口时是否直接退出（false = 隐藏到托盘常驻）
static CLOSE_TO_QUIT: AtomicBool = AtomicBool::new(false);
// 全局偏好：状态栏是否显示网络流量
static SHOW_NET: AtomicBool = AtomicBool::new(true);

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppSettings {
    #[serde(default)]
    close_to_quit: bool,
    #[serde(default = "default_true")]
    show_net: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            close_to_quit: false,
            show_net: true,
        }
    }
}

// 偏好设置持久化到缓存目录下的 settings.json
fn settings_path() -> Option<PathBuf> {
    CACHE_DIR_PATH.as_ref().map(|dir| dir.join("settings.json"))
}

fn load_settings() -> AppSettings {
    settings_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

fn save_settings(settings: &AppSettings) {
    if let Some(path) = settings_path() {
        match serde_json::to_string_pretty(settings) {
            Ok(content) => {
                if let Err(e) = std::fs::write(path, content) {
                    eprintln!("Failed to save settings: {}", e);
                }
            }
            Err(e) => eprintln!("Failed to serialize settings: {}", e),
        }
    }
}

// 更新并持久化单项偏好
fn update_setting(show_net: Option<bool>, close_to_quit: Option<bool>) {
    let mut settings = load_settings();
    if let Some(v) = show_net {
        settings.show_net = v;
    }
    if let Some(v) = close_to_quit {
        settings.close_to_quit = v;
    }
    save_settings(&settings);
}

// 全局复用的 System 实例：sysinfo 的 CPU 占用依赖同一实例两次刷新的差值，
// 且重建全量进程表代价高，不应每次轮询都重新创建
lazy_static::lazy_static! {
    // sysinfo 0.40 起 System::new() 返回 Result，仅在 sysinfo 不支持的平台上
    // 失败；本项目目标平台（Windows/macOS/Linux）均在支持范围内
    static ref SYSTEM: Mutex<System> =
        Mutex::new(System::new().expect("failed to initialize sysinfo System"));
}

// 系统网络吞吐统计：sysinfo 的 NetworkData 是距上次刷新的增量，
// 结合两次刷新的间隔换算为每秒速率
struct NetRateState {
    // sysinfo 0.40 的 Networks::new() 返回 Result，初始化失败时置 None
    // （极少见的平台不支持场景），网速统计退化为全零
    networks: Option<Networks>,
    last_refresh: Option<Instant>,
    down_bps: u64,
    up_bps: u64,
    total_down: u64,
    total_up: u64,
}

lazy_static::lazy_static! {
    static ref NET_RATE: Mutex<NetRateState> = Mutex::new(NetRateState {
        networks: Networks::new().ok(),
        last_refresh: None,
        down_bps: 0,
        up_bps: 0,
        total_down: 0,
        total_up: 0,
    });
}

// 刷新系统网络吞吐并计算速率与会话累计（回环接口不计入）
fn update_net_rate() -> NetRate {
    let mut net = NET_RATE.lock().unwrap_or_else(|p| p.into_inner());
    let Some(networks) = net.networks.as_mut() else {
        return NetRate::default();
    };

    // sysinfo 0.40 起 refresh(参数) 一次性完成网卡列表重建与计数刷新；
    // true 会清除已移除的网卡（替代旧版每 30 次一轮的 refresh_list 轮换）
    networks.refresh(true);

    let mut down_delta = 0u64;
    let mut up_delta = 0u64;
    for (name, data) in networks.iter() {
        let lower = name.to_string_lossy().to_lowercase();
        // 排除回环与常见虚拟网卡（虚拟交换机/容器网桥会镜像物理网卡流量，
        // 直接相加会重复计数）。命名覆盖三平台：
        // Windows: vEthernet (WSL/Hyper-V)；Linux: lo/veth*/docker0/br-*；
        // macOS: lo0。VPN 隧道（utun/wg/tailscale）是真实流量，保留计数。
        if lower == "lo"
            || lower.starts_with("lo0")
            || lower.contains("loopback")
            || lower.starts_with("vethernet")
            || lower.starts_with("veth")
            || lower.starts_with("docker")
            || lower.starts_with("br-")
            || lower.contains("virtual")
            || lower.contains("vmware")
            || lower.starts_with("tap-")
            || lower.starts_with("wan miniport")
        {
            continue;
        }
        down_delta += data.received();
        up_delta += data.transmitted();
    }

    // 会话累计：直接累加每次刷新的增量，不依赖 sysinfo 的 total 语义
    net.total_down += down_delta;
    net.total_up += up_delta;

    // 速率 = 增量 / 实际间隔；刷新过近时沿用上次速率避免抖动
    let (down_bps, up_bps) = match net.last_refresh {
        Some(last) => {
            let secs = last.elapsed().as_secs_f64();
            if secs < 0.05 {
                (net.down_bps, net.up_bps)
            } else {
                (
                    (down_delta as f64 / secs) as u64,
                    (up_delta as f64 / secs) as u64,
                )
            }
        }
        None => (0, 0),
    };

    net.down_bps = down_bps;
    net.up_bps = up_bps;
    net.last_refresh = Some(Instant::now());
    NetRate {
        down_bps,
        up_bps,
        total_down: net.total_down,
        total_up: net.total_up,
    }
}

// 缓存进程图标信息，包含上次更新时间，以提高性能
// 键是进程路径的MD5值，值是(图标数据, 时间戳, 是否有图标)的元组
lazy_static::lazy_static! {
    static ref ICON_CACHE: Mutex<HashMap<String, (Option<String>, SystemTime, bool)>> = Mutex::new(HashMap::new());
}

// 初始化缓存目录
fn initialize_cache_directory() -> Option<PathBuf> {
    // 使用用户的 home 目录下的 .portview 目录
    let cache_dir = dirs::home_dir().map(|home| home.join(".portview"));

    let cache_dir = match cache_dir {
        Some(dir) => dir,
        None => {
            eprintln!("Failed to get home directory");
            return None;
        }
    };

    eprintln!("Cache directory: {:?}", cache_dir);

    // 创建缓存目录（如果不存在）
    match std::fs::create_dir_all(&cache_dir) {
        Ok(_) => {
            eprintln!("Successfully created cache directory: {:?}", cache_dir);
            Some(cache_dir)
        }
        Err(e) => {
            eprintln!("Failed to create cache directory {:?}: {}", cache_dir, e);
            None
        }
    }
}

// 获取缓存目录路径
fn get_cache_directory() -> Option<PathBuf> {
    CACHE_DIR_PATH.clone()
}

// 通用的获取进程图标函数（使用缓存）
fn get_process_icon_by_path(exe_path: &str) -> Option<String> {
    // 使用路径的MD5值作为缓存键
    let cache_key = format!("{:x}", md5::compute(exe_path.as_bytes()));

    // 一次获取锁，检查缓存
    {
        let cache = ICON_CACHE.lock().unwrap_or_else(|p| p.into_inner());
        if let Some((cached_icon, _, has_cached)) = cache.get(&cache_key) {
            // 如果之前已经缓存了"无图标"的结果，则直接返回None
            if !has_cached {
                return None;
            }
            // 返回缓存的图标
            return cached_icon.clone();
        }
    }

    // 使用路径的MD5值作为缓存文件名
    let icon_filename = format!("{}.png", cache_key);

    // 获取预初始化的缓存目录
    let cache_dir = get_cache_directory()?;

    // 检查是否已经为该进程路径生成过缓存文件
    let png_cache_path = cache_dir.join(icon_filename);

    // 如果缓存文件存在，直接使用它
    if png_cache_path.exists() {
        if let Ok(png_data) = std::fs::read(&png_cache_path) {
            let base64_icon = base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &png_data,
            );

            // 将图标添加到内存缓存
            let mut cache = ICON_CACHE.lock().unwrap_or_else(|p| p.into_inner());
            cache.insert(
                cache_key,
                (Some(base64_icon.clone()), SystemTime::now(), true),
            );

            return Some(base64_icon);
        }
    }

    // 缓存文件不存在，提取图标；失败时统一在此写负缓存。
    // 不能依赖平台实现来记录失败结果：它们存在提前返回的路径（如
    // Windows 图标转换失败、macOS 的 `?` 传播），会绕过各自的负缓存写入
    let png_data = match extract_icon_from_exe(exe_path) {
        Some(data) => data,
        None => {
            let mut cache = ICON_CACHE.lock().unwrap_or_else(|p| p.into_inner());
            cache.insert(cache_key, (None, SystemTime::now(), false));
            return None;
        }
    };

    // 保存转换后的PNG到缓存目录
    let base64_icon = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &png_data,
    );

    // 尝试保存到文件缓存
    let cache_result = std::fs::write(&png_cache_path, &png_data);

    // 将图标添加到内存缓存
    let mut cache = ICON_CACHE.lock().unwrap_or_else(|p| p.into_inner());
    cache.insert(
        cache_key,
        (Some(base64_icon.clone()), SystemTime::now(), true),
    );

    // 如果文件缓存失败，记录警告
    if cache_result.is_err() {
        eprintln!("Warning: Failed to write icon cache file: {:?}", png_cache_path);
    }

    Some(base64_icon)
}

// 从可执行文件提取图标（平台特定实现）
fn extract_icon_from_exe(exe_path: &str) -> Option<Vec<u8>> {
    #[cfg(target_os = "windows")]
    {
        extract_icon_from_exe_windows(exe_path)
    }

    #[cfg(target_os = "macos")]
    {
        extract_icon_from_exe_macos(exe_path)
    }

    #[cfg(target_os = "linux")]
    {
        extract_icon_from_exe_linux(exe_path)
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

// ==================== Windows 平台实现 ====================

#[cfg(target_os = "windows")]
fn extract_icon_from_exe_windows(exe_path: &str) -> Option<Vec<u8>> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;
    use winapi::shared::minwindef::HINSTANCE;
    use winapi::um::shellapi::ExtractIconW;
    use winapi::um::winuser::DestroyIcon;

    // 将路径转换为宽字符字符串
    let wide_path: Vec<u16> = OsStr::new(exe_path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // 使用ExtractIconW从EXE文件中提取图标
    unsafe {
        let h_instance = ptr::null_mut() as HINSTANCE;
        let h_icon = ExtractIconW(
            h_instance,
            wide_path.as_ptr(),
            0, // 第一个图标
        );

        // 检查图标句柄是否有效
        if h_icon as usize > 1 {
            // 0和1是特殊值，表示没有图标或错误
            // 尝试将图标转换为图像数据
            let icon_data = extract_icon_to_png(h_icon);

            // 销毁图标句柄
            DestroyIcon(h_icon);

            return icon_data;
        }
    }

    None
}

// 辅助函数：将图标转换为PNG数据
#[cfg(target_os = "windows")]
use winapi::shared::windef::HICON;

#[cfg(target_os = "windows")]
unsafe fn extract_icon_to_png(h_icon: HICON) -> Option<Vec<u8>> {
    use std::mem;
    use std::ptr;
    use winapi::um::wingdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDIBits, SelectObject,
        BI_RGB, DIB_RGB_COLORS,
    };
    use winapi::um::winnt::HANDLE;
    use winapi::um::winuser::{DrawIconEx, GetDC, GetIconInfo, ReleaseDC};

    // 获取图标信息
    let mut icon_info: winapi::um::winuser::ICONINFO = mem::zeroed();
    if GetIconInfo(h_icon, &mut icon_info) == 0 {
        return None;
    }

    // 获取屏幕DC
    let hdc_screen = GetDC(ptr::null_mut());
    if hdc_screen.is_null() {
        // 清理资源
        if !icon_info.hbmColor.is_null() {
            DeleteObject(icon_info.hbmColor as *mut _);
        }
        if !icon_info.hbmMask.is_null() {
            DeleteObject(icon_info.hbmMask as *mut _);
        }
        return None;
    }

    // 创建兼容DC
    let hdc_mem = CreateCompatibleDC(hdc_screen);
    if hdc_mem.is_null() {
        ReleaseDC(ptr::null_mut(), hdc_screen);
        if !icon_info.hbmColor.is_null() {
            DeleteObject(icon_info.hbmColor as *mut _);
        }
        if !icon_info.hbmMask.is_null() {
            DeleteObject(icon_info.hbmMask as *mut _);
        }
        return None;
    }

    // 选择位图到内存DC
    let hbm_old = SelectObject(hdc_mem, icon_info.hbmColor as *mut _);

    // 获取颜色位图信息
    let mut bitmap: winapi::um::wingdi::BITMAP = mem::zeroed();
    if winapi::um::wingdi::GetObjectW(
        icon_info.hbmColor as HANDLE,
        mem::size_of::<winapi::um::wingdi::BITMAP>() as i32,
        &mut bitmap as *mut _ as *mut _,
    ) == 0
    {
        SelectObject(hdc_mem, hbm_old);
        DeleteDC(hdc_mem);
        ReleaseDC(ptr::null_mut(), hdc_screen);
        if !icon_info.hbmColor.is_null() {
            DeleteObject(icon_info.hbmColor as *mut _);
        }
        if !icon_info.hbmMask.is_null() {
            DeleteObject(icon_info.hbmMask as *mut _);
        }
        return None;
    }

    let width = bitmap.bmWidth;
    let height = bitmap.bmHeight.abs();

    // 准备BITMAPINFO结构
    let mut bmi: winapi::um::wingdi::BITMAPINFO = mem::zeroed();
    bmi.bmiHeader.biSize = mem::size_of::<winapi::um::wingdi::BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = width;
    bmi.bmiHeader.biHeight = height;
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;

    // 创建DIB Section
    let mut bits: *mut winapi::ctypes::c_void = ptr::null_mut();
    let h_bitmap = CreateDIBSection(hdc_mem, &bmi, DIB_RGB_COLORS, &mut bits, ptr::null_mut(), 0);

    if h_bitmap.is_null() {
        SelectObject(hdc_mem, hbm_old);
        DeleteDC(hdc_mem);
        ReleaseDC(ptr::null_mut(), hdc_screen);
        if !icon_info.hbmColor.is_null() {
            DeleteObject(icon_info.hbmColor as *mut _);
        }
        if !icon_info.hbmMask.is_null() {
            DeleteObject(icon_info.hbmMask as *mut _);
        }
        return None;
    }

    // 选择新位图到内存DC
    let hbm_old_dib = SelectObject(hdc_mem, h_bitmap as *mut _);

    // 将原图标绘制到位图上
    DrawIconEx(
        hdc_mem,
        0,
        0,
        h_icon,
        width,
        height,
        0,
        ptr::null_mut(),
        0x0003, // DI_NORMAL
    );

    // 获取DIB位
    let buffer_size = (width * height * 4) as usize;
    let mut buffer: Vec<u8> = vec![0; buffer_size];

    let nbits = GetDIBits(
        hdc_mem,
        h_bitmap,
        0,
        height as u32,
        buffer.as_mut_ptr() as *mut _,
        &mut bmi,
        DIB_RGB_COLORS,
    );

    // 恢复DC状态
    SelectObject(hdc_mem, hbm_old_dib);
    SelectObject(hdc_mem, hbm_old);

    // 清理资源
    DeleteObject(h_bitmap as *mut _);
    DeleteDC(hdc_mem);
    ReleaseDC(ptr::null_mut(), hdc_screen);

    if nbits != 0 {
        // 将BGRA转换为RGBA（Windows使用BGRA，而PNG使用RGBA）
        for chunk in buffer.chunks_exact_mut(4) {
            chunk.swap(0, 2); // 交换B和R通道
        }

        // 由于Windows DIB格式的Y轴方向与标准图像相反，需要垂直翻转图像
        let row_size = (width * 4) as usize;
        let mut flipped_buffer = Vec::with_capacity(buffer.len());

        // 从最后一行开始复制到新缓冲区，实现垂直翻转
        for row in (0..height).rev() {
            let start = (row * width * 4) as usize;
            let end = start + row_size;
            flipped_buffer.extend_from_slice(&buffer[start..end]);
        }

        // 将图标数据转换为PNG格式
        use image::ImageFormat;
        use std::io::Cursor;

        if let Some(img) = image::RgbaImage::from_raw(width as u32, height as u32, flipped_buffer) {
            let mut png_data: Vec<u8> = Vec::new();
            if img.write_to(&mut Cursor::new(&mut png_data), ImageFormat::Png).is_ok() {
                if !icon_info.hbmColor.is_null() {
                    DeleteObject(icon_info.hbmColor as *mut _);
                }
                if !icon_info.hbmMask.is_null() {
                    DeleteObject(icon_info.hbmMask as *mut _);
                }

                return Some(png_data);
            }
        }
    }

    // 清理资源
    if !icon_info.hbmColor.is_null() {
        DeleteObject(icon_info.hbmColor as *mut _);
    }
    if !icon_info.hbmMask.is_null() {
        DeleteObject(icon_info.hbmMask as *mut _);
    }

    None
}

// ==================== macOS 平台实现 ====================

#[cfg(target_os = "macos")]
fn extract_icon_from_exe_macos(exe_path: &str) -> Option<Vec<u8>> {
    // 通过字符串操作直接查找路径中的 .app 部分并构建 Resources 路径
    let app_index = exe_path.rfind(".app/");
    if let Some(index) = app_index {
        // 提取 .app 目录路径
        let app_path = &exe_path[..index + 4]; // 包括 ".app"
        let resources_path = format!("{}/Contents/Resources", app_path);

        if std::path::Path::new(&resources_path).exists() {
            // 在 Resources 目录中查找 .icns 文件（只在第一层）
            let resources_dir = std::path::PathBuf::from(resources_path);
            let icns_path = {
                let mut icns_file_path = None;
                if let Ok(entries) = std::fs::read_dir(&resources_dir) {
                    for entry in entries {
                        if let Ok(entry) = entry {
                            let file_name = entry.file_name();
                            if let Some(name_str) = file_name.to_str() {
                                if name_str.to_lowercase().ends_with(".icns") {
                                    icns_file_path = Some(entry.path());
                                    break; // 只取第一个找到的 .icns 文件
                                }
                            }
                        }
                    }
                }
                icns_file_path?
            };

            // 转换ICNS到PNG
            return convert_icns_to_png(icns_path.to_str()?).ok();
        }
    }

    None
}

// macOS辅助函数：将ICNS转换为PNG
#[cfg(target_os = "macos")]
fn convert_icns_to_png(icns_path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use std::process::Command;
    use tempfile::NamedTempFile;

    // 创建临时文件用于输出PNG
    let temp_file = NamedTempFile::new()?;
    let temp_path = temp_file.path().to_str().ok_or("Invalid temp file path")?;

    eprintln!("Converting ICNS to PNG: {} -> {}", icns_path, temp_path);

    // 使用sips命令将ICNS转换为PNG
    // 注意：sips命令参数顺序很重要，源文件应该在最后
    let output = Command::new("sips")
        .args(&[
            "-s", "format", "png",     // 设置输出格式为PNG
            icns_path, // 输入文件
            "-o", temp_path, // 输出文件
        ])
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "sips command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    // 读取临时文件内容
    let png_data = std::fs::read(temp_path)?;

    eprintln!("Conversion successful, PNG size: {} bytes", png_data.len());

    if png_data.is_empty() {
        return Err("Converted PNG data is empty".into());
    }

    Ok(png_data)
}

// ==================== Linux 平台实现 ====================

#[cfg(target_os = "linux")]
fn extract_icon_from_exe_linux(exe_path: &str) -> Option<Vec<u8>> {
    use std::path::Path;

    // 从可执行文件路径提取进程名
    let basename = Path::new(exe_path)
        .file_name()?
        .to_string_lossy()
        .to_string();

    // 尝试多种可能的图标路径
    let icon_sizes = [
        "16x16", "24x24", "32x32", "48x48", "64x64", "128x128", "256x256", "scalable",
    ];
    let icon_themes = ["hicolor", "oxygen", "gnome", "breeze"];
    let icon_types = ["apps", "categories", "devices", "mimetypes"];

    // 首先尝试桌面文件中指定的图标
    let desktop_file_path = format!("/usr/share/applications/{}.desktop", &basename);
    if Path::new(&desktop_file_path).exists() {
        if let Ok(desktop_content) = std::fs::read_to_string(&desktop_file_path) {
            for line in desktop_content.lines() {
                if line.starts_with("Icon=") {
                    let icon_name = line.strip_prefix("Icon=").unwrap_or("");
                    if !icon_name.is_empty() {
                        // 尝试查找该图标名称
                        for size in &icon_sizes {
                            for theme in &icon_themes {
                                for icon_type in &icon_types {
                                    let icon_path = format!(
                                        "/usr/share/icons/{}/{}/{}/{}.png",
                                        theme, size, icon_type, icon_name
                                    );
                                    if Path::new(&icon_path).exists() {
                                        if let Ok(image_data) = std::fs::read(&icon_path) {
                                            return Some(image_data);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 如果桌面文件没有帮助，尝试基于进程名查找图标
    for size in &icon_sizes {
        for theme in &icon_themes {
            for icon_type in &icon_types {
                let icon_paths = vec![
                    format!(
                        "/usr/share/icons/{}/{}/{}/{}.png",
                        theme, size, icon_type, &basename
                    ),
                    format!(
                        "/usr/share/icons/{}/{}/{}/{}.svg",
                        theme, size, icon_type, &basename
                    ),
                    format!("/usr/share/pixmaps/{}.png", &basename),
                    format!("/usr/share/pixmaps/{}.svg", &basename),
                ];

                for icon_path in icon_paths {
                    if Path::new(&icon_path).exists() {
                        if let Ok(image_data) = std::fs::read(&icon_path) {
                            return Some(image_data);
                        }
                    }
                }
            }
        }
    }

    None
}

// ==================== 网络连接相关函数 ====================

// 根据 PID 从进程表中组装单条连接记录（TCP/UDP 共用）
fn build_connection(
    protocol: &str,
    local_addr: String,
    local_port: u16,
    remote_addr: String,
    remote_port: u16,
    state: &str,
    pid: Option<u32>,
    process_map: &HashMap<Pid, &sysinfo::Process>,
) -> TcpConnection {
    let process_info = pid.and_then(|p| process_map.get(&Pid::from_u32(p)));

    let process_name = match process_info {
        // sysinfo 0.40 起 name() 返回 &OsStr，lossy 转 UTF-8
        Some(process) => Some(process.name().to_string_lossy().into_owned()),
        // 如果无法获取进程信息，可能是内核进程或权限不足，显示特殊标识
        None if pid.is_some() => Some("[KERNEL]".to_string()),
        None => None,
    };

    let exe_path = process_info
        .and_then(|process| process.exe())
        .map(|path| path.to_string_lossy().to_string());

    let start_time = match process_info {
        Some(process) => Some(process.start_time()),
        // 内核进程的时间信息可能不可用，返回0
        None if pid.is_some() => Some(0),
        None => None,
    };

    TcpConnection {
        protocol: protocol.to_string(),
        local_addr,
        local_port,
        remote_addr,
        remote_port,
        state: state.to_string(),
        pid,
        process_name,
        exe_path,
        start_time,
    }
}

// 获取系统网络连接列表
#[tauri::command]
async fn get_connections() -> Result<ConnectionsSnapshot, String> {
    // 设置地址族标志 (IPv4 和 IPv6)
    let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    // 设置协议标志 (TCP 和 UDP)
    let proto_flags = ProtocolFlags::TCP | ProtocolFlags::UDP;

    // 获取网络连接信息
    let sockets_info = get_sockets_info(af_flags, proto_flags).map_err(|e| e.to_string())?;

    // 在全局 System 上做增量刷新，而不是每次调用都重建全量进程表；
    // 同一实例的两次刷新间隔也让进程的 CPU 占用有了计算基准
    let mut system = SYSTEM.lock().unwrap_or_else(|p| p.into_inner());
    system.refresh_processes(ProcessesToUpdate::All, true);

    // 预先构建进程信息映射表，避免逐个 socket 重复查询
    let process_map: HashMap<Pid, &sysinfo::Process> = system
        .processes()
        .iter()
        .map(|(pid, process)| (*pid, process))
        .collect();

    let mut connections = Vec::with_capacity(sockets_info.len());
    for si in sockets_info {
        let pid = si.associated_pids.first().copied();
        match si.protocol_socket_info {
            ProtocolSocketInfo::Tcp(tcp_si) => connections.push(build_connection(
                "TCP",
                tcp_si.local_addr.to_string(),
                tcp_si.local_port,
                tcp_si.remote_addr.to_string(),
                tcp_si.remote_port,
                tcp_state_to_string(tcp_si.state),
                pid,
                &process_map,
            )),
            ProtocolSocketInfo::Udp(udp_si) => connections.push(build_connection(
                "UDP",
                udp_si.local_addr.to_string(),
                udp_si.local_port,
                "*".to_string(),
                0,
                "UNCONN",
                pid,
                &process_map,
            )),
        }
    }
    // 图标提取可能较慢（GDI/文件IO），先释放全局锁再进行
    drop(system);

    // 每个唯一的可执行文件路径只提取并传输一次图标
    let mut icons: HashMap<String, String> = HashMap::new();
    let exe_paths: HashSet<&str> = connections
        .iter()
        .filter_map(|conn| conn.exe_path.as_deref())
        .collect();
    for exe_path in exe_paths {
        if let Some(icon) = get_process_icon_by_path(exe_path) {
            icons.insert(exe_path.to_string(), icon);
        }
    }

    // 刷新系统网络吞吐
    let net_rate = update_net_rate();

    Ok(ConnectionsSnapshot { connections, icons, net_rate })
}

// 获取系统网络吞吐速率（轻量命令：前端独立定时器轮询，不依赖自动刷新开关）
#[tauri::command]
async fn get_net_rate() -> Result<NetRate, String> {
    Ok(update_net_rate())
}

// sysinfo 的 cmd() 是按 argv 解析的（CommandLineToArgvW），原始引号已被剥离；
// 重组时给含空格的参数补回引号，避免参数内部的空格与参数分隔符混淆。
// sysinfo 0.40 起参数为 OsString，统一 lossy 转成 UTF-8 字符串处理
fn process_command_line(process: &sysinfo::Process) -> String {
    process
        .cmd()
        .iter()
        .map(|arg| {
            let arg = arg.to_string_lossy();
            if arg.contains(' ') || arg.contains('\t') {
                format!("\"{}\"", arg.replace('"', "\\\""))
            } else {
                arg.into_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// 从 sysinfo 进程条目组装详情（含命令行重组）
fn build_process_details(pid: u32, process: &sysinfo::Process) -> ProcessDetails {
    let command_line = process_command_line(process);

    ProcessDetails {
        pid,
        name: process.name().to_string_lossy().into_owned(),
        command_line,
        executable_path: process
            .exe()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_default(),
        memory_usage: process.memory(),
        cpu_usage: process.cpu_usage(),
        parent_pid: process.parent().map(|p| p.as_u32()),
        start_time: process.start_time(),
    }
}

// 获取进程详情
#[tauri::command]
async fn get_process_details(pid: u32) -> Result<ProcessDetails, String> {
    // 复用全局 System；只刷新目标进程——列表轮询每秒已做全量刷新。
    // 注意两点：①sysinfo 的刷新按字段开关，命令行必须显式指定
    // （cmd 用 OnlyIfNotSet，进程存活期间只需读取一次）；②单进程刷新
    // 失败不代表进程不存在——权限受限的进程（如 Windows 的 System/pid 4）
    // 刷不动，但列表轮询的缓存里仍有它的数据，此时回退使用缓存。
    let refresh_kind = ProcessRefreshKind::nothing()
        .with_memory()
        .with_cpu()
        .with_cmd(UpdateKind::OnlyIfNotSet)
        .with_exe(UpdateKind::OnlyIfNotSet);

    let mut system = SYSTEM.lock().unwrap_or_else(|p| p.into_inner());
    let pid_typed = Pid::from_u32(pid);
    // sysinfo 0.40 起返回更新的进程数而非 bool；remove_dead 传 false，
    // 保证刷新失败时轮询缓存的条目不被移除（下方回退要用）
    if system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid_typed]),
        false,
        refresh_kind,
    ) == 0
        && system.process(pid_typed).is_none()
    {
        return Err(format!("Process with PID {} not found", pid));
    }

    match system.process(pid_typed) {
        Some(process) => Ok(build_process_details(pid, process)),
        None => Err(format!("Process with PID {} not found", pid)),
    }
}

// 从 sysinfo 进程条目组装调用链节点（用户名靠 Users 列表解析；
// 列表读取失败时为 None，节点的 user 字段退化为空）
fn build_lineage_node(
    pid: u32,
    process: &sysinfo::Process,
    parent_exited: bool,
    is_target: bool,
    users: Option<&Users>,
) -> LineageNode {
    let user = users.and_then(|users| {
        process
            .user_id()
            .and_then(|uid| users.get_user_by_id(uid))
            .map(|u| u.name().to_string())
    });

    LineageNode {
        pid,
        ppid: process.parent().map(|p| p.as_u32()),
        name: process.name().to_string_lossy().into_owned(),
        command_line: process_command_line(process),
        exe_path: process
            .exe()
            .map(|path| path.to_string_lossy().to_string()),
        start_time: Some(process.start_time()),
        user,
        icon: None, // 图标提取涉及文件 IO，统一在锁外按 exe_path 补齐
        parent_exited,
        is_target,
    }
}

// 获取进程调用链：从目标进程沿 PPID 迭代向上直到 PID 1/0，
// 每一跳都带三重防护——环检测、PID 回收检测、父进程消失检测。
// 调用链基本静态（进程几乎不会换父进程），前端打开抽屉时拉取一次即可
#[tauri::command]
async fn get_process_lineage(pid: u32) -> Result<ProcessLineage, String> {
    let refresh_kind = ProcessRefreshKind::nothing()
        .with_memory()
        .with_cmd(UpdateKind::OnlyIfNotSet)
        .with_exe(UpdateKind::OnlyIfNotSet);

    let mut system = SYSTEM.lock().unwrap_or_else(|p| p.into_inner());
    // 用户名解析依赖 Users 列表，调用一次全量刷新（开销可接受）；
    // 读取失败时退化为 None，节点 user 字段为空但不影响调用链
    let users = Users::new_with_refreshed_list().ok();

    let mut chain_rev: Vec<LineageNode> = Vec::new();
    let mut seen: HashSet<u32> = HashSet::new();
    let mut current = pid;
    // 下一跳父进程的启动时间必须早于它，否则说明 PPID 槽位被回收复用
    let mut child_start_time: Option<u64> = None;

    loop {
        // PID 0 为根；seen 防环：异常 PPID 链（如内核数据竞争）直接终止
        if current == 0 || !seen.insert(current) {
            break;
        }
        let pid_typed = Pid::from_u32(current);
        // 单进程刷新失败（返回 0）不代表不存在：权限受限的进程回退用
        // 轮询缓存里的数据；remove_dead 传 false 保证缓存条目不被移除
        if system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid_typed]),
            false,
            refresh_kind,
        ) == 0
            && system.process(pid_typed).is_none()
        {
            // 父进程确实读不到（已退出且无缓存）：在子节点上标记断裂
            if let Some(last) = chain_rev.last_mut() {
                last.parent_exited = true;
            }
            break;
        }
        let Some(process) = system.process(pid_typed) else {
            break;
        };

        // PID 回收防护：父进程启动晚于子进程，说明原父进程已退出、
        // 该 PPID 被新进程复用——不能把无关进程拼进链里，到此为止
        let start_time = process.start_time();
        if let Some(child_start) = child_start_time {
            if start_time > child_start {
                if let Some(last) = chain_rev.last_mut() {
                    last.parent_exited = true;
                }
                break;
            }
        }

        child_start_time = Some(start_time);
        chain_rev.push(build_lineage_node(
            current,
            process,
            false,
            current == pid,
            users.as_ref(),
        ));

        // ppid 拿不到（如部分内核进程）时终止
        match process.parent() {
            Some(p) => current = p.as_u32(),
            None => break,
        }
    }
    // 反转成 root → target 顺序
    let mut nodes: Vec<LineageNode> = chain_rev;
    nodes.reverse();

    // 直接子进程：从已有进程表过滤即可（全量刷新由连接列表的秒级轮询负责）
    let target_pid = Pid::from_u32(pid);
    let mut children: Vec<LineageNode> = system
        .processes()
        .iter()
        .filter(|(_, process)| process.parent() == Some(target_pid))
        .map(|(child_pid, process)| {
            build_lineage_node(child_pid.as_u32(), process, false, false, users.as_ref())
        })
        .collect();
    // 按 PID 排序保证展示稳定，并限制数量避免极端进程组（如浏览器多进程）刷屏
    children.sort_by_key(|node| node.pid);
    children.truncate(50);

    // 图标提取可能较慢（文件 IO / icns 转换），先释放全局锁再进行
    drop(system);
    for node in nodes.iter_mut().chain(children.iter_mut()) {
        if let Some(exe_path) = node.exe_path.as_deref() {
            node.icon = get_process_icon_by_path(exe_path);
        }
    }

    Ok(ProcessLineage { nodes, children })
}

// 实时解析某个端口当前的所有绑定（TCP/UDP、任意状态）及属主进程。
// 供菜单栏"追踪"入口使用：不依赖前端快照的新鲜度，自动刷新关闭时也能查
#[tauri::command]
async fn get_port_bindings(port: u16) -> Result<Vec<PortBinding>, String> {
    let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    let proto_flags = ProtocolFlags::TCP | ProtocolFlags::UDP;
    let sockets_info = get_sockets_info(af_flags, proto_flags).map_err(|e| e.to_string())?;

    // 刷新进程表拿进程名（与连接列表同一套增量刷新）
    let mut system = SYSTEM.lock().unwrap_or_else(|p| p.into_inner());
    system.refresh_processes(ProcessesToUpdate::All, true);

    let mut bindings = Vec::new();
    for si in sockets_info {
        let (protocol, local_addr, local_port, state) = match si.protocol_socket_info {
            ProtocolSocketInfo::Tcp(tcp_si) => (
                "TCP",
                tcp_si.local_addr.to_string(),
                tcp_si.local_port,
                tcp_state_to_string(tcp_si.state).to_string(),
            ),
            ProtocolSocketInfo::Udp(udp_si) => (
                "UDP",
                udp_si.local_addr.to_string(),
                udp_si.local_port,
                "UNCONN".to_string(),
            ),
        };
        if local_port != port {
            continue;
        }
        let pid = si.associated_pids.first().copied();
        let process_name = pid
            .and_then(|p| system.process(Pid::from_u32(p)))
            .map(|process| process.name().to_string_lossy().into_owned());
        bindings.push(PortBinding {
            protocol: protocol.to_string(),
            local_addr,
            state,
            pid,
            process_name,
        });
    }
    Ok(bindings)
}

// 杀死进程
#[tauri::command]
async fn kill_process(pid: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::process::Command;

        // 尝试使用 SIGTERM 优雅地终止进程
        let output = Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .output()
            .map_err(|e| format!("Failed to execute kill command: {}", e))?;

        if !output.status.success() {
            // 如果 SIGTERM 失败，尝试使用 SIGKILL 强制终止
            let output = Command::new("kill")
                .arg("-KILL")
                .arg(pid.to_string())
                .output()
                .map_err(|e| format!("Failed to execute kill command: {}", e))?;

            if !output.status.success() {
                return Err(format!("Failed to kill process with PID {}", pid));
            }
        }

        Ok(())
    }

    #[cfg(windows)]
    {
        use winapi::um::handleapi::CloseHandle;
        use winapi::um::processthreadsapi::OpenProcess;
        use winapi::um::processthreadsapi::TerminateProcess;
        use winapi::um::winnt::PROCESS_TERMINATE;

        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
            if handle.is_null() {
                return Err(format!("Failed to open process with PID {}", pid));
            }

            let result = TerminateProcess(handle, 1);
            CloseHandle(handle);

            if result != 0 {
                Ok(())
            } else {
                Err(format!("Failed to kill process with PID {}", pid))
            }
        }
    }
}

// 打开文件所在目录
#[tauri::command]
async fn open_folder(path: String) -> Result<(), String> {
    use std::path::Path;
    use std::process::Command;

    // 确保我们获取的是文件所在的目录，而不是文件本身
    let path_obj = Path::new(&path);
    let dir = if path_obj.is_file() {
        // 如果是文件路径，获取其父目录
        path_obj.parent().unwrap_or(Path::new(&path))
    } else {
        // 如果已经是目录路径，直接使用
        path_obj
    };

    let dir_str = dir.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(&dir_str)
            .spawn()
            .map_err(|e| format!("Failed to open explorer: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&dir_str)
            .spawn()
            .map_err(|e| format!("Failed to open finder: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&dir_str)
            .spawn()
            .map_err(|e| format!("Failed to open file manager: {}", e))?;
    }

    Ok(())
}

// 前端启动时查询偏好设置（托盘可切换的开关项）
#[tauri::command]
async fn get_app_settings() -> Result<AppSettings, String> {
    Ok(load_settings())
}

// 获取应用版本号
#[tauri::command]
async fn get_app_version() -> Result<String, String> {
    let version = env!("CARGO_PKG_VERSION");
    Ok(version.to_string())
}

// 更新窗口主题的备用实现（当平台特定代码不可用时）
#[tauri::command]
async fn update_window_theme(_window: tauri::Window, _is_dark_mode: bool) -> Result<(), String> {
    // 目前，由于平台API限制，我们无法动态更改原生窗口标题栏的外观
    // 这是一个占位符函数，用于前端调用，但不执行任何操作
    Ok(())
}

// 唤起并聚焦主窗口
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

// 切换开机自启（托盘菜单勾选项）
fn toggle_autostart(app: &tauri::AppHandle) {
    use tauri_plugin_autostart::ManagerExt;

    let manager = app.autolaunch();
    let enabled = manager.is_enabled().unwrap_or(false);
    let _ = if enabled {
        manager.disable()
    } else {
        manager.enable()
    };
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 单实例：重复启动时唤起已有主窗口（官方要求注册为第一个插件）
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if CLOSE_TO_QUIT.load(Ordering::Relaxed) {
                    // 「关闭即退出」开启：不阻止关闭，窗口关闭后应用随之退出
                } else {
                    // 默认：隐藏到托盘常驻，退出走托盘菜单
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            use tauri::menu::{CheckMenuItem, MenuBuilder, MenuItem, PredefinedMenuItem};
            use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

            let handle = app.handle();

            // 加载偏好设置（关闭行为、网络流量展示等）
            let settings = load_settings();
            CLOSE_TO_QUIT.store(settings.close_to_quit, Ordering::Relaxed);
            SHOW_NET.store(settings.show_net, Ordering::Relaxed);

            let show_item =
                MenuItem::with_id(handle, "show", "显示 PortView", true, None::<&str>)?;
            let show_net_item = CheckMenuItem::with_id(
                handle,
                "show_net",
                "显示网络流量",
                true,
                settings.show_net,
                None::<&str>,
            )?;
            let close_quit_item = CheckMenuItem::with_id(
                handle,
                "close_quit",
                "关闭窗口即退出",
                true,
                settings.close_to_quit,
                None::<&str>,
            )?;
            // 初始勾选状态跟随系统当前的自启配置
            let autostart_checked = {
                use tauri_plugin_autostart::ManagerExt;
                handle.autolaunch().is_enabled().unwrap_or(false)
            };
            let autostart_item = CheckMenuItem::with_id(
                handle,
                "autostart",
                "开机自启",
                true,
                autostart_checked,
                None::<&str>,
            )?;
            let quit_item = MenuItem::with_id(handle, "quit", "退出", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(handle)?;
            let menu = MenuBuilder::new(handle)
                .items(&[
                    &show_item,
                    &show_net_item,
                    &close_quit_item,
                    &autostart_item,
                    &separator,
                    &quit_item,
                ])
                .build()?;

            let icon = handle.default_window_icon().cloned().ok_or_else(|| {
                tauri::Error::AssetNotFound("default window icon".to_string())
            })?;

            TrayIconBuilder::with_id("portview-tray")
                .icon(icon)
                .tooltip("PortView")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => show_main_window(app),
                    "show_net" => {
                        // 勾选状态由菜单自动切换，这里同步偏好、持久化并通知前端
                        let enabled = !SHOW_NET.load(Ordering::Relaxed);
                        SHOW_NET.store(enabled, Ordering::Relaxed);
                        update_setting(Some(enabled), None);
                        let _ = app.emit("show-net-changed", enabled);
                    }
                    "close_quit" => {
                        // 勾选状态由菜单自动切换，这里同步偏好并持久化
                        let enabled = !CLOSE_TO_QUIT.load(Ordering::Relaxed);
                        CLOSE_TO_QUIT.store(enabled, Ordering::Relaxed);
                        update_setting(None, Some(enabled));
                    }
                    "autostart" => toggle_autostart(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    // 左键单击或双击托盘图标：唤起主窗口；右键弹出菜单
                    match event {
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        }
                        | TrayIconEvent::DoubleClick { .. } => {
                            show_main_window(tray.app_handle());
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_connections,
            get_net_rate,
            get_process_details,
            get_process_lineage,
            get_port_bindings,
            get_app_settings,
            kill_process,
            open_folder,
            get_app_version,
            update_window_theme
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
