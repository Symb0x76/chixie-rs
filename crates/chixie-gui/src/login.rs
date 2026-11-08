//! Login screen. The password only goes to the session worker (for this request) and, if
//! "remember" is ticked, to the OS credential store — never to settings or logs.

use chixie_core::credentials;
use chixie_core::session::Command;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{ActiveTheme as _, StyledExt as _, v_flex};
use gpui_kit::*;

use crate::state::AppState;

pub struct LoginView {
    account: Entity<InputState>,
    password: Entity<InputState>,
    remember: bool,
    busy: bool,
    error: Option<String>,
    /// (account, password) of the request in flight, needed to persist "remember me" on success.
    pending: Option<(String, String)>,
    _subs: Vec<Subscription>,
}

impl LoginView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let st = cx.global::<AppState>();
        let saved_account = st.settings.account.clone().unwrap_or_default();
        let remember = st.settings.remember_password;
        let saved_password = if remember && !saved_account.is_empty() {
            credentials::load(&saved_account).unwrap_or_else(|e| {
                log::warn!("credential store: {e}");
                None
            })
        } else {
            None
        };
        let account = cx.new(|cx| {
            InputState::new(window, cx).placeholder("手机号 / 邮箱").default_value(saved_account)
        });
        let password = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("密码")
                .masked(true)
                .default_value(saved_password.unwrap_or_default())
        });
        let sub = cx.subscribe_in(&password, window, |this, _state, ev: &InputEvent, _window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.submit(cx);
            }
        });
        Self { account, password, remember, busy: false, error: None, pending: None, _subs: vec![sub] }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let account = self.account.read(cx).value().trim().to_string();
        let password = self.password.read(cx).value().to_string();
        self.submit_values(account, password, self.remember, cx);
    }

    /// Start a login with explicit values (also used by the autotest).
    pub fn submit_values(&mut self, account: String, password: String, remember: bool, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if account.is_empty() || password.is_empty() {
            self.error = Some("请输入账号和密码".into());
            cx.notify();
            return;
        }
        self.remember = remember;
        self.busy = true;
        self.error = None;
        self.pending = Some((account.clone(), password.clone()));
        cx.global::<AppState>().session.send(Command::Login { account, password });
        cx.notify();
    }

    pub fn on_logged_in(&mut self, cx: &mut Context<Self>) {
        self.busy = false;
        if let Some((account, password)) = self.pending.take() {
            let remember = self.remember;
            let r = if remember { credentials::save(&account, &password) } else { credentials::forget(&account) };
            if let Err(e) = r {
                log::warn!("credential store: {e}");
            }
            let st = cx.global_mut::<AppState>();
            st.settings.account = Some(account);
            st.settings.remember_password = remember;
            st.save_settings();
        }
        cx.notify();
    }

    pub fn on_failed(&mut self, msg: String, cx: &mut Context<Self>) {
        self.busy = false;
        self.pending = None;
        // Backend messages look like "api: user/login: HTTP 400 Bad Request: <msg>"; show <msg>.
        self.error = Some(msg.rsplit(": ").next().unwrap_or(&msg).to_string());
        cx.notify();
    }
}

impl Render for LoginView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .bg(theme.background)
            .child(
                v_flex()
                    .w(px(360.))
                    .gap_3()
                    .p_6()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme.border)
                    .child(div().text_xl().font_semibold().child("赤写 ChiXie"))
                    .child(div().text_sm().text_color(theme.muted_foreground).child("登录以使用云端字体"))
                    .child(Input::new(&self.account).cleanable(true))
                    .child(Input::new(&self.password).mask_toggle())
                    .child(
                        Checkbox::new("remember")
                            .label(if cfg!(target_os = "windows") {
                                "记住密码（保存在 Windows 凭据管理器）"
                            } else if cfg!(target_os = "macos") {
                                "记住密码（保存在钥匙串）"
                            } else {
                                "记住密码（保存在系统密钥环）"
                            })
                            .checked(self.remember)
                            .on_click(cx.listener(|this, checked: &bool, _w, cx| {
                                this.remember = *checked;
                                cx.notify();
                            })),
                    )
                    .children(self.error.clone().map(|e| div().text_sm().text_color(theme.danger).child(e)))
                    .child(
                        Button::new("login")
                            .primary()
                            .label("登录")
                            .loading(self.busy)
                            .on_click(cx.listener(|this, _ev: &ClickEvent, _w, cx| this.submit(cx))),
                    ),
            )
    }
}
