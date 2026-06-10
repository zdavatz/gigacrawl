// Signal messaging via presage — the Rust client stack built on the official
// libsignal crates. This machine is linked to the user's existing Signal
// account as a secondary device (like Signal Desktop); the protocol state
// lives in `signal_store.db3` (cwd then $HOME, gitignored — treat it like the
// LinkedIn/X token files: it can send messages as the user).
//
// Flow: `--signal-link` once (QR scan with the phone), `--signal-groups` to
// find the target group's master key, `--post-signal <group>` to send the PDF.
// Groups appear in the store only after a message in them has been synced —
// if a group is missing, have someone post in it and run `--signal-groups`
// again.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use futures::{channel::oneshot, future, pin_mut, StreamExt};
use presage::libsignal_service::configuration::SignalServers;
use presage::libsignal_service::content::{ContentBody, DataMessage, GroupContextV2};
use presage::proto::data_message::Delete;
use presage::libsignal_service::sender::AttachmentSpec;
use presage::libsignal_service::zkgroup::GroupMasterKeyBytes;
use presage::manager::Registered;
use presage::model::identity::OnNewIdentity;
use presage::model::messages::Received;
use presage::store::ContentsStore;
use presage::Manager;
use presage_store_sqlite::SqliteStore;

type Error = Box<dyn std::error::Error>;

const STORE_FILE: &str = "signal_store.db3";
const DEVICE_NAME: &str = "gigacrawl";

/// cwd first, then $HOME — same lookup convention as the LinkedIn/X creds.
/// A fresh link always creates the store in cwd.
fn store_path() -> String {
    if Path::new(STORE_FILE).exists() {
        return STORE_FILE.to_string();
    }
    if let Some(h) = std::env::var_os("HOME") {
        let p = Path::new(&h).join(STORE_FILE);
        if p.exists() {
            return p.display().to_string();
        }
    }
    STORE_FILE.to_string()
}

/// presage is async; the rest of gigacrawl is blocking. Each public entry
/// point spins up a current-thread runtime + LocalSet (presage's receive task
/// is spawn_local'd, mirroring its own CLI).
fn block_on<F, T>(fut: F) -> T
where
    F: std::future::Future<Output = T>,
{
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let local = tokio::task::LocalSet::new();
    rt.block_on(local.run_until(fut))
}

async fn open_store() -> Result<SqliteStore, Error> {
    Ok(SqliteStore::open_with_passphrase(&store_path(), None, OnNewIdentity::Trust).await?)
}

/// Drain the incoming message queue once (until the server reports it empty).
/// Processing the queue is what populates contacts and groups in the store.
async fn sync_until_empty(manager: &mut Manager<SqliteStore, Registered>) -> Result<(), Error> {
    let messages = manager.receive_messages().await?;
    pin_mut!(messages);
    while let Some(received) = messages.next().await {
        match received {
            Received::QueueEmpty => break,
            Received::Contacts => println!("[signal] contacts synced"),
            Received::Content(_) => {}
        }
    }
    Ok(())
}

/// `--signal-link`: register this machine as a secondary device of the user's
/// Signal account. Prints a QR code to scan with the phone (Signal → Settings
/// → Linked Devices → +), then drains the queue so groups/contacts land in
/// the store.
pub fn link_device() -> Result<(), Error> {
    block_on(async {
        let store = open_store().await?;
        let (tx, rx) = oneshot::channel();
        let (manager, _) = future::join(
            Manager::link_secondary_device(
                store,
                SignalServers::Production,
                DEVICE_NAME.to_string(),
                tx,
            ),
            async move {
                match rx.await {
                    Ok(url) => {
                        // Render the QR to a PNG and open it in the default
                        // viewer — terminal QR rendering depends on the
                        // emulator's ANSI handling, a PNG always works.
                        let png = std::env::temp_dir().join("signal_link_qr.png");
                        match qrcode::QrCode::new(url.to_string().as_bytes()) {
                            Ok(code) => {
                                let img = code
                                    .render::<image::Luma<u8>>()
                                    .min_dimensions(400, 400)
                                    .build();
                                match img.save(&png) {
                                    Ok(()) => {
                                        println!(
                                            "QR code written to {} — opening it; scan it with \
                                             Signal on your phone (Settings → Linked Devices → +).",
                                            png.display()
                                        );
                                        let _ = open::that(&png);
                                    }
                                    Err(e) => eprintln!("[signal] QR PNG save failed: {e}"),
                                }
                            }
                            Err(e) => eprintln!("[signal] QR encoding failed: {e}"),
                        }
                        let _ = qr2term::print_qr(url.to_string());
                        println!("(If the QR is unusable: the raw provisioning URL is)\n{url}");
                    }
                    Err(e) => eprintln!("[signal] linking cancelled: {e}"),
                }
            },
        )
        .await;
        let mut manager = manager?;
        println!("[signal] linked as \"{DEVICE_NAME}\" — syncing message queue (can take a minute)…");
        sync_until_empty(&mut manager).await?;
        // Ask the primary device for a contact sync, then drain again so the
        // contacts (and any groups referenced meanwhile) get stored.
        if manager.request_contacts().await.is_ok() {
            sync_until_empty(&mut manager).await?;
        }
        println!("[signal] done. Next: `--signal-groups` to find your group.");
        Ok(())
    })
}

/// `--signal-groups`: drain the queue, then list every group in the store as
/// `<hex master key>  <title>`.
pub fn list_groups() -> Result<(), Error> {
    block_on(async {
        let store = open_store().await?;
        let mut manager = Manager::load_registered(store).await?;
        sync_until_empty(&mut manager).await?;
        let mut n = 0;
        for group in manager.store().groups().await? {
            let (key, group) = group?;
            println!("{}  {}", hex::encode(key), group.title);
            n += 1;
        }
        if n == 0 {
            println!("[signal] no groups in the store yet — groups are learned from synced \
                      messages, so post (or have someone post) in the group and rerun this.");
        }
        Ok(())
    })
}

/// Resolve `which` to a group master key: either 64 hex chars, or a
/// case-insensitive substring of a stored group title (must be unambiguous).
async fn resolve_group(
    manager: &Manager<SqliteStore, Registered>,
    which: &str,
) -> Result<(GroupMasterKeyBytes, String), Error> {
    if which.len() == 64 {
        if let Ok(bytes) = hex::decode(which) {
            let key: GroupMasterKeyBytes =
                bytes.try_into().map_err(|_| "master key must be 32 bytes")?;
            return Ok((key, which.to_string()));
        }
    }
    let needle = which.to_lowercase();
    let mut hits = Vec::new();
    for group in manager.store().groups().await? {
        let (key, group) = group?;
        if group.title.to_lowercase().contains(&needle) {
            hits.push((key, group.title));
        }
    }
    match hits.len() {
        1 => Ok(hits.pop().unwrap()),
        0 => Err(format!("no group matching \"{which}\" — run --signal-groups").into()),
        _ => Err(format!(
            "\"{which}\" is ambiguous: {}",
            hits.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join(", ")
        )
        .into()),
    }
}

/// `--post-signal <group> [message]`: upload the PDF as an attachment and send
/// it with the caption to the resolved group.
pub fn send_pdf_to_group(which: &str, pdf_path: &Path, caption: &str) -> Result<String, Error> {
    let data = std::fs::read(pdf_path)?;
    let file_name = pdf_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string());
    block_on(async {
        let store = open_store().await?;
        let mut manager = Manager::load_registered(store).await?;
        // Drain the queue first: sessions/sender keys must be current before
        // sending, and this also refreshes the group store.
        sync_until_empty(&mut manager).await?;
        let (master_key, title) = resolve_group(&manager, which).await?;

        let spec = AttachmentSpec {
            content_type: "application/pdf".to_string(),
            length: data.len(),
            file_name,
            preview: None,
            voice_note: None,
            borderless: None,
            width: None,
            height: None,
            caption: None,
            blur_hash: None,
        };
        let attachments: Result<Vec<_>, _> = manager
            .upload_attachments(vec![(spec, data)])
            .await?
            .into_iter()
            .collect();
        let attachments = attachments?;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_millis() as u64;
        let message = DataMessage {
            body: Some(caption.to_string()),
            attachments,
            timestamp: Some(timestamp),
            group_v2: Some(GroupContextV2 {
                master_key: Some(master_key.to_vec()),
                revision: Some(0),
                ..Default::default()
            }),
            ..Default::default()
        };
        println!(
            "[signal] sending: {} attachment(s), timestamp {timestamp}",
            message.attachments.len()
        );
        manager
            .send_message_to_group(&master_key, ContentBody::DataMessage(message), timestamp)
            .await?;
        println!("[signal] sent at timestamp {timestamp} (needed for --signal-delete)");
        Ok(title)
    })
}

/// `--signal-messages <group>`: dump the stored thread — timestamp, attachment
/// count and body per message. Debug aid (e.g. to find a timestamp to delete).
pub fn list_messages(which: &str) -> Result<(), Error> {
    block_on(async {
        let store = open_store().await?;
        let mut manager = Manager::load_registered(store).await?;
        sync_until_empty(&mut manager).await?;
        let (master_key, title) = resolve_group(&manager, which).await?;
        println!("[signal] thread \"{title}\":");
        let messages = manager
            .store()
            .messages(&presage::store::Thread::Group(master_key), 0..)
            .await?;
        for msg in messages.filter_map(Result::ok) {
            let ts = msg.metadata.timestamp;
            match &msg.body {
                ContentBody::DataMessage(dm) => println!(
                    "{ts}  attachments={}  body={:?}",
                    dm.attachments.len(),
                    dm.body.as_deref().unwrap_or("")
                ),
                ContentBody::SynchronizeMessage(sync) => {
                    if let Some(dm) = sync.sent.as_ref().and_then(|s| s.message.as_ref()) {
                        println!(
                            "{ts}  attachments={}  body={:?}  (synced from another device)",
                            dm.attachments.len(),
                            dm.body.as_deref().unwrap_or("")
                        );
                    }
                }
                other => println!("{ts}  ({})", { let _ = other; "non-data message" }),
            }
        }
        Ok(())
    })
}

/// `--signal-delete <group> <timestamp>`: remote-delete ("delete for
/// everyone") a message this account sent to the group.
pub fn delete_group_message(which: &str, target_ts: u64) -> Result<(), Error> {
    block_on(async {
        let store = open_store().await?;
        let mut manager = Manager::load_registered(store).await?;
        sync_until_empty(&mut manager).await?;
        let (master_key, title) = resolve_group(&manager, which).await?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_millis() as u64;
        let message = DataMessage {
            delete: Some(Delete {
                target_sent_timestamp: Some(target_ts),
            }),
            timestamp: Some(timestamp),
            group_v2: Some(GroupContextV2 {
                master_key: Some(master_key.to_vec()),
                revision: Some(0),
                ..Default::default()
            }),
            ..Default::default()
        };
        manager
            .send_message_to_group(&master_key, ContentBody::DataMessage(message), timestamp)
            .await?;
        println!("[signal] delete-for-everyone of {target_ts} sent to \"{title}\"");
        Ok(())
    })
}
