#[path = "media_job_priority.rs"]
mod priority;
use std::{
    collections::HashMap,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
#[derive(Clone)]
pub(crate) struct Runtime {
    pub flags: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    pub active: Arc<AtomicUsize>,
    pub stopping: Arc<AtomicBool>,
    pub wake: Arc<tokio::sync::Notify>,
    pub loudness_changed: Arc<(Mutex<u64>, Condvar)>,
    pub collection_changed: Arc<(Mutex<u64>, Condvar)>,
    pub proxy_changed: Arc<(Mutex<u64>, Condvar)>,
    pub audio_changed: Arc<(Mutex<u64>, Condvar)>,
    pub render_changed: Arc<(Mutex<u64>, Condvar)>,
    pub video_previews: Arc<Mutex<crate::image_probe::video_preview::cache::Cache>>,
    pub slots: Arc<priority::Slots>,
}
impl Default for Runtime {
    fn default() -> Self {
        Self {
            flags: Default::default(),
            active: Default::default(),
            stopping: Default::default(),
            wake: Default::default(),
            loudness_changed: Default::default(),
            collection_changed: Default::default(),
            proxy_changed: Default::default(),
            audio_changed: Default::default(),
            render_changed: Default::default(),
            slots: Default::default(),
            video_previews: Default::default(),
        }
    }
}
impl Runtime {
    pub fn notify_collection_changed(&self) {
        let mut generation = self
            .collection_changed
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *generation = generation.wrapping_add(1);
        self.collection_changed.1.notify_all();
    }
    pub fn notify_proxy_changed(&self) {
        let mut generation = self
            .proxy_changed
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *generation = generation.wrapping_add(1);
        self.proxy_changed.1.notify_all();
    }
    pub fn notify_loudness_changed(&self) {
        let mut generation = self
            .loudness_changed
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *generation = generation.wrapping_add(1);
        self.loudness_changed.1.notify_all();
    }
    pub fn notify_audio_changed(&self) {
        let mut generation = self
            .audio_changed
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *generation = generation.wrapping_add(1);
        self.audio_changed.1.notify_all();
    }
    pub fn notify_render_changed(&self) {
        let mut generation = self
            .render_changed
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *generation = generation.wrapping_add(1);
        self.render_changed.1.notify_all();
    }
    pub async fn shutdown(&self) {
        self.stopping.store(true, Ordering::SeqCst);
        self.slots.interrupt_background();
        for flag in self
            .flags
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
        {
            flag.store(true, Ordering::SeqCst);
        }
        while self.active.load(Ordering::SeqCst) != 0 {
            tokio::select! { _ = self.wake.notified() => {}, _ = tokio::time::sleep(Duration::from_millis(50)) => {} }
        }
        self.video_previews
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }
}
