use tauri::WebviewWindow;

/// Ajusta a janela para um utilitário flutuante.
///
/// O que a aplicação controla: tamanho fixo, sem maximizar, sempre acima,
/// centralizada na abertura e, no macOS, nível e collection behavior da NSWindow.
/// O que depende do sistema: um window manager de tiling pode ignorar essas
/// dicas. No Windows o equivalente é HWND_TOPMOST. No Linux, o GTK marca
/// keep-above; i3, Sway e Hyprland decidem sozinhos se a janela entra no tile.
pub fn configure_utility_window(window: &WebviewWindow) {
    let _ = window.set_resizable(false);
    let _ = window.set_maximizable(false);
    let _ = window.set_always_on_top(true);
    let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize {
        width: 300.0,
        height: 420.0,
    }));
    let _ = window.center();
    configure_native(window);
}

#[cfg(target_os = "macos")]
fn configure_native(window: &WebviewWindow) {
    use objc2_app_kit::{
        NSFloatingWindowLevel, NSWindow, NSWindowCollectionBehavior,
    };

    let Ok(raw) = window.ns_window() else {
        return;
    };
    let Some(ptr) = std::ptr::NonNull::new(raw as *mut NSWindow) else {
        return;
    };
    let ns_window = unsafe { ptr.as_ref() };
    ns_window.setLevel(NSFloatingWindowLevel);
    ns_window.setHidesOnDeactivate(false);
    ns_window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Transient
            | NSWindowCollectionBehavior::IgnoresCycle
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::FullScreenDisallowsTiling,
    );
}

#[cfg(not(target_os = "macos"))]
fn configure_native(_window: &WebviewWindow) {}
