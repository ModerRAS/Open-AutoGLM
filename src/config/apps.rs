//! App name to package name mapping for supported applications.

use once_cell::sync::Lazy;
use std::collections::HashMap;

/// Mapping from app display names to Android package names.
pub static APP_PACKAGES: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    
    // Social & Messaging
    m.insert("微信", "com.tencent.mm");
    m.insert("QQ", "com.tencent.mobileqq");
    m.insert("微博", "com.sina.weibo");
    
    // E-commerce
    m.insert("淘宝", "com.taobao.taobao");
    m.insert("京东", "com.jingdong.app.mall");
    m.insert("拼多多", "com.xunmeng.pinduoduo");
    m.insert("淘宝闪购", "com.taobao.taobao");
    m.insert("京东秒送", "com.jingdong.app.mall");
    
    // Lifestyle & Social
    m.insert("小红书", "com.xingin.xhs");
    m.insert("豆瓣", "com.douban.frodo");
    m.insert("知乎", "com.zhihu.android");
    
    // Maps & Navigation
    m.insert("高德地图", "com.autonavi.minimap");
    m.insert("百度地图", "com.baidu.BaiduMap");
    
    // Food & Services
    m.insert("美团", "com.sankuai.meituan");
    m.insert("大众点评", "com.dianping.v1");
    m.insert("饿了么", "me.ele");
    m.insert("肯德基", "com.yek.android.kfc.activitys");
    
    // Travel
    m.insert("携程", "ctrip.android.view");
    m.insert("铁路12306", "com.MobileTicket");
    m.insert("12306", "com.MobileTicket");
    m.insert("去哪儿", "com.Qunar");
    m.insert("去哪儿旅行", "com.Qunar");
    m.insert("滴滴出行", "com.sdu.did.psnger");
    
    // Video & Entertainment
    m.insert("bilibili", "tv.danmaku.bili");
    m.insert("抖音", "com.ss.android.ugc.aweme");
    m.insert("快手", "com.smile.gifmaker");
    m.insert("腾讯视频", "com.tencent.qqlive");
    m.insert("爱奇艺", "com.qiyi.video");
    m.insert("优酷视频", "com.youku.phone");
    m.insert("芒果TV", "com.hunantv.imgo.activity");
    m.insert("红果短剧", "com.phoenix.read");
    
    // Music & Audio
    m.insert("网易云音乐", "com.netease.cloudmusic");
    m.insert("QQ音乐", "com.tencent.qqmusic");
    m.insert("汽水音乐", "com.luna.music");
    m.insert("喜马拉雅", "com.ximalaya.ting.android");
    
    // Reading
    m.insert("番茄小说", "com.dragon.read");
    m.insert("番茄免费小说", "com.dragon.read");
    m.insert("七猫免费小说", "com.kmxs.reader");
    
    // Productivity
    m.insert("飞书", "com.ss.android.lark");
    m.insert("QQ邮箱", "com.tencent.androidqqmail");
    
    // AI & Tools
    m.insert("豆包", "com.larus.nova");
    
    // Health & Fitness
    m.insert("keep", "com.gotokeep.keep");
    m.insert("美柚", "com.lingan.seeyou");
    
    // News & Information
    m.insert("腾讯新闻", "com.tencent.news");
    m.insert("今日头条", "com.ss.android.article.news");
    
    // Real Estate
    m.insert("贝壳找房", "com.lianjia.beike");
    m.insert("安居客", "com.anjuke.android.app");
    
    // Finance
    m.insert("同花顺", "com.hexin.plat.android");
    
    // Games
    m.insert("星穹铁道", "com.miHoYo.hkrpg");
    m.insert("崩坏：星穹铁道", "com.miHoYo.hkrpg");
    m.insert("恋与深空", "com.papegames.lysk.cn");
    
    // System
    m.insert("AndroidSystemSettings", "com.android.settings");
    m.insert("Android System Settings", "com.android.settings");
    m.insert("Android  System Settings", "com.android.settings");
    m.insert("Android-System-Settings", "com.android.settings");
    m.insert("Settings", "com.android.settings");
    
    // Common apps (English names)
    m.insert("AudioRecorder", "com.android.soundrecorder");
    m.insert("audiorecorder", "com.android.soundrecorder");
    m.insert("Bluecoins", "com.rammigsoftware.bluecoins");
    m.insert("bluecoins", "com.rammigsoftware.bluecoins");
    m.insert("Broccoli", "com.flauschcode.broccoli");
    m.insert("broccoli", "com.flauschcode.broccoli");
    m.insert("Booking.com", "com.booking");
    m.insert("Booking", "com.booking");
    m.insert("booking.com", "com.booking");
    m.insert("booking", "com.booking");
    m.insert("BOOKING.COM", "com.booking");
    m.insert("Chrome", "com.android.chrome");
    m.insert("chrome", "com.android.chrome");
    m.insert("Google Chrome", "com.android.chrome");
    m.insert("Clock", "com.android.deskclock");
    m.insert("clock", "com.android.deskclock");
    m.insert("Contacts", "com.android.contacts");
    m.insert("contacts", "com.android.contacts");
    m.insert("Duolingo", "com.duolingo");
    m.insert("duolingo", "com.duolingo");
    m.insert("Expedia", "com.expedia.bookings");
    m.insert("expedia", "com.expedia.bookings");
    m.insert("Files", "com.android.fileexplorer");
    m.insert("files", "com.android.fileexplorer");
    m.insert("File Manager", "com.android.fileexplorer");
    m.insert("Gmail", "com.google.android.gm");
    m.insert("gmail", "com.google.android.gm");
    m.insert("Google Maps", "com.google.android.apps.maps");
    m.insert("Maps", "com.google.android.apps.maps");
    m.insert("YouTube", "com.google.android.youtube");
    m.insert("youtube", "com.google.android.youtube");
    m.insert("Camera", "com.android.camera");
    m.insert("camera", "com.android.camera");
    m.insert("Gallery", "com.android.gallery3d");
    m.insert("gallery", "com.android.gallery3d");
    m.insert("Calculator", "com.android.calculator2");
    m.insert("calculator", "com.android.calculator2");
    m.insert("Calendar", "com.android.calendar");
    m.insert("calendar", "com.android.calendar");
    m.insert("Messages", "com.android.mms");
    m.insert("messages", "com.android.mms");
    m.insert("Phone", "com.android.dialer");
    m.insert("phone", "com.android.dialer");
    
    m
});

/// Get the package name for an app by its display name.
pub fn get_package(app_name: &str) -> Option<&'static str> {
    APP_PACKAGES.get(app_name).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_package() {
        assert_eq!(get_package("微信"), Some("com.tencent.mm"));
        assert_eq!(get_package("Chrome"), Some("com.android.chrome"));
        assert_eq!(get_package("NonExistent"), None);
    }
}
