use eframe::egui;

use crate::app::Myapp;
use common::taskmanager::TaskManager_debug;

const REPO_URL: &str = "https://github.com/biliticket/bili_ticket_rush";
const ISSUES_URL: &str = "https://github.com/biliticket/bili_ticket_rush/issues";

struct HelpPalette {
    card_fill: egui::Color32,
    card_border: egui::Color32,
    text_primary: egui::Color32,
    text_secondary: egui::Color32,
    text_muted: egui::Color32,
    accent: egui::Color32,
    slate: egui::Color32,
    notice_fill: egui::Color32,
    notice_border: egui::Color32,
    notice_text: egui::Color32,
    badge_fill: egui::Color32,
    badge_text: egui::Color32,
}

impl HelpPalette {
    fn from_dark_mode(dark_mode: bool) -> Self {
        if dark_mode {
            Self {
                card_fill: egui::Color32::from_rgb(36, 38, 46),
                card_border: egui::Color32::from_rgb(62, 66, 80),
                text_primary: egui::Color32::from_rgb(235, 238, 245),
                text_secondary: egui::Color32::from_rgb(186, 192, 208),
                text_muted: egui::Color32::from_rgb(142, 148, 166),
                accent: egui::Color32::from_rgb(45, 136, 245),
                slate: egui::Color32::from_rgb(82, 94, 118),
                notice_fill: egui::Color32::from_rgb(56, 44, 30),
                notice_border: egui::Color32::from_rgb(160, 108, 48),
                notice_text: egui::Color32::from_rgb(255, 194, 122),
                badge_fill: egui::Color32::from_rgb(52, 58, 74),
                badge_text: egui::Color32::from_rgb(180, 212, 255),
            }
        } else {
            Self {
                card_fill: egui::Color32::from_rgb(248, 249, 252),
                card_border: egui::Color32::from_rgb(214, 218, 230),
                text_primary: egui::Color32::from_rgb(32, 36, 48),
                text_secondary: egui::Color32::from_rgb(76, 82, 98),
                text_muted: egui::Color32::from_rgb(122, 128, 144),
                accent: egui::Color32::from_rgb(45, 136, 245),
                slate: egui::Color32::from_rgb(92, 104, 128),
                notice_fill: egui::Color32::from_rgb(255, 247, 235),
                notice_border: egui::Color32::from_rgb(240, 186, 122),
                notice_text: egui::Color32::from_rgb(188, 86, 20),
                badge_fill: egui::Color32::from_rgb(228, 238, 252),
                badge_text: egui::Color32::from_rgb(25, 92, 188),
            }
        }
    }
}

fn help_card(
    ui: &mut egui::Ui,
    title: &str,
    palette: &HelpPalette,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::none()
        .fill(palette.card_fill)
        .rounding(12.0)
        .stroke(egui::Stroke::new(1.0, palette.card_border))
        .inner_margin(egui::Margin::symmetric(16.0, 14.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width().max(260.0));
            ui.label(
                egui::RichText::new(title)
                    .size(16.0)
                    .strong()
                    .color(palette.text_primary),
            );
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(8.0);
            add_contents(ui);
        });
}

fn guide_step(
    ui: &mut egui::Ui,
    step_title: &str,
    body: &str,
    palette: &HelpPalette,
) {
    ui.label(
        egui::RichText::new(step_title)
            .size(14.5)
            .strong()
            .color(palette.text_primary),
    );
    ui.add_space(3.0);
    ui.add(
        egui::Label::new(
            egui::RichText::new(body)
                .size(13.0)
                .color(palette.text_secondary),
        )
        .wrap(true),
    );
}

pub fn render(app: &mut Myapp, ui: &mut egui::Ui) {
    let dark_mode = ui.ctx().style().visuals.dark_mode;
    let palette = HelpPalette::from_dark_mode(dark_mode);

    // 顶部标题与快捷操作按钮
    ui.horizontal(|ui| {
        ui.heading(
            egui::RichText::new("帮助 / 关于")
                .size(22.0)
                .strong()
                .color(palette.text_primary),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let log_btn_label = if app.show_log_window {
                "📜 收起实时日志"
            } else {
                "📜 打开实时日志窗口"
            };
            let log_btn = egui::Button::new(
                egui::RichText::new(log_btn_label)
                    .size(13.5)
                    .strong()
                    .color(egui::Color32::WHITE),
            )
            .fill(if app.show_log_window {
                egui::Color32::from_rgb(16, 152, 104)
            } else {
                palette.accent
            })
            .rounding(8.0)
            .min_size(egui::vec2(146.0, 31.0));
            if ui.add(log_btn).clicked() {
                app.show_log_window = !app.show_log_window;
            }

            ui.add_space(8.0);

            let account_btn = egui::Button::new(
                egui::RichText::new("跳转到账号管理")
                    .size(13.0)
                    .strong()
                    .color(egui::Color32::WHITE),
            )
            .fill(palette.slate)
            .rounding(7.0)
            .min_size(egui::vec2(114.0, 31.0));
            if ui.add(account_btn).clicked() {
                app.selected_tab = 2;
            }
        });
    });

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(6.0);

    egui::ScrollArea::vertical()
        .id_source("help_tab_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // 若存在官方公告（announce2），置顶显示
            if let Some(announce) = app
                .announce2
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                egui::Frame::none()
                    .fill(palette.notice_fill)
                    .rounding(10.0)
                    .stroke(egui::Stroke::new(1.0, palette.notice_border))
                    .inner_margin(egui::Margin::symmetric(16.0, 12.0))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width().max(260.0));
                        ui.label(
                            egui::RichText::new("📢 官方公告")
                                .size(15.5)
                                .strong()
                                .color(palette.notice_text),
                        );
                        ui.add_space(6.0);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(announce)
                                    .size(13.5)
                                    .color(palette.notice_text),
                            )
                            .wrap(true),
                        );
                    });
                ui.add_space(12.0);
            }

            // 应用信息与开源免责声明
            help_card(ui, "关于本软件", &palette, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(10.0, 6.0);
                    ui.label(
                        egui::RichText::new(format!("BiliTicketGUI ({})", app.app))
                            .size(16.0)
                            .strong()
                            .color(palette.text_primary),
                    );
                    egui::Frame::none()
                        .fill(palette.badge_fill)
                        .rounding(7.0)
                        .inner_margin(egui::Margin::symmetric(8.0, 2.5))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(format!("v{}", app.display_version))
                                    .size(12.5)
                                    .strong()
                                    .color(palette.badge_text),
                            );
                        });
                });

                ui.add_space(8.0);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(TaskManager_debug())
                            .size(13.5)
                            .color(palette.notice_text)
                            .strong(),
                    )
                    .wrap(true),
                );

                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(16.0, 6.0);
                    ui.hyperlink_to("🔗 GitHub 项目主页", REPO_URL);
                    ui.hyperlink_to("🐛 问题反馈 (GitHub Issues)", ISSUES_URL);
                });
            });

            ui.add_space(12.0);

            // 完整使用指南
            help_card(ui, "使用指南", &palette, |ui| {
                guide_step(
                    ui,
                    "1. 账号登录与防黄牛限制（最多 2 个账号）",
                    "前往「账号管理」点击「+ 添加账号」，支持 B 站 App 扫码登录、短信验证码登录与 Cookie 登录。为防止项目被黄牛滥用，最多仅支持同时保存 2 个账号；可通过每个账号卡片上的「抢票开启中 / 抢票关闭中」按钮单独控制是否允许参与抢票。",
                    &palette,
                );
                ui.add_space(10.0);

                guide_step(
                    ui,
                    "2. 添加购票人信息",
                    "实名制票务项目需提前准备好常用购票人：在「账号管理」对应账号下点击「添加购票人」即可新增并同步至账号。非实名制项目可在「设置 / 微调」中配置账号预填手机号，确认下单时将自动填充。",
                    &palette,
                );
                ui.add_space(10.0);

                guide_step(
                    ui,
                    "3. 开始抢票与三种模式说明",
                    "在「开始抢票」页输入 5~10 位项目 ID 或粘贴完整的 B 站票务链接（不支持 b23.tv 短链），选择账号与模式后按回车或点击「开始抢票」：\n\
                     • 定时抢票（推荐）：自动读取开售时间并在开票瞬间触发下单。\n\
                     • 直接抢票：跳过倒计时立即尝试下单，仅适合已开票项目，未开票项目使用可能触发账号风控。\n\
                     • 捡漏模式：针对已开票/暂时售罄项目，按设定间隔持续监测回流余票并在出现余票时自动下单。",
                    &palette,
                );
                ui.add_space(10.0);

                guide_step(
                    ui,
                    "4. 推送通知与自动保存说明",
                    "在「设置 / 微调」中开启推送开关后，可配置 Bark、PushPlus、方糖（Server酱）、钉钉机器人、企业微信机器人、Gotify 以及 SMTP 邮件通知。SMTP 支持 465 端口（隐式 TLS）和 587 端口（STARTTLS），授权密码处请填写邮箱提供的 SMTP 专用授权码。所有设置项在离开输入框、松开滑块或切换开关后会自动保存并立即生效，无需手动点击保存按钮。",
                    &palette,
                );
                ui.add_space(10.0);

                guide_step(
                    ui,
                    "5. 常见问题与跨平台运行提示",
                    "• 支付超时提醒：抢票成功并弹出支付二维码后，请在订单有效期内（通常为 15 分钟）尽快扫码或在 B 站 App「会员购订单」中完成付款，超时订单会自动取消。\n\
                     • 日志排查：如遇登录失败、风控提示或网络超时，可随时点击左侧栏底部「📜 打开实时日志」或页面右上角按钮查看完整报错，程序同时会将本地日志文件保存在数据目录的 Log/ 文件夹中。\n\
                     • 防风控建议：可在「设置 / 微调」中适当调大捡漏轮询间隔与下单重试间隔，避免短时间高频请求触发限制。\n\
                     • macOS / Linux 提示：macOS 通过 .app 启动时会自动将数据保存在 ~/Library/Application Support/BiliTicketRush；Linux 若无硬件 OpenGL 3.3 驱动，可设置环境变量 BILI_FORCE_SOFTWARE_GL=1 启用软件渲染。",
                    &palette,
                );
            });

            ui.add_space(12.0);

            ui.label(
                egui::RichText::new("提示：本软件仅用于学习与个人辅助，请遵守平台规则与相关法律法规。")
                    .size(12.0)
                    .color(palette.text_muted),
            );
            ui.add_space(16.0);
        });
}
