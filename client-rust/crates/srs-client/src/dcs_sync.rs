//! Applies DCS events to the client state: the logic of
//! `DCSRadioSyncHandler.ProcessRadioInfoAsync`, `DCSGameGuiHandler`,
//! `DCSLineOfSightHandler` (results), `UDPCommandHandler` and
//! `DCSRadioSyncManager.CheckIfRadioIsStale`. Runs on its own thread.

use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use srs_dcs::merge::{merge, MergeContext, MergeEvent, MergeMemory};
use srs_dcs::radio_helper as rh;
use srs_net::{NetCommand, SharedState};
use srs_protocol::LatLng;

use crate::client::{build_client, AppEvent};
use crate::combined_state::build_line;
use crate::dcs_io::{DcsEvent, DcsIo, UdpCommandType};
use crate::radio_state::SharedRadioState;
use crate::state::ClientState;

/// Where network commands go (the TCP session, or a test sink).
pub type NetSink = Arc<dyn Fn(NetCommand) + Send + Sync>;

/// `RADIO_UPDATE_PING_INTERVAL`: resend the full state at least this often.
pub const RADIO_UPDATE_PING_INTERVAL: Duration = Duration::from_secs(60);

pub struct DcsSyncCtx {
    pub radio_state: SharedRadioState,
    pub shared: Arc<SharedState>,
    pub state: Arc<Mutex<ClientState>>,
    pub merge_ctx: Arc<Mutex<MergeContext>>,
    pub net: NetSink,
    pub dcs_io: Arc<DcsIo>,
    pub app_events: mpsc::Sender<AppEvent>,
    pub wake: Arc<dyn Fn() + Send + Sync>,
    /// Profile `RotaryStyleIncrement`, for UDP frequency commands.
    pub rotary_style: Arc<std::sync::atomic::AtomicBool>,
    /// Preset channel lists per radio (index 0 unused), for channel commands.
    pub fixed_channels: Arc<Mutex<Vec<Vec<rh::PresetChannel>>>>,
}

/// Loop until the event channel closes.
pub fn run_dcs_sync(rx: mpsc::Receiver<DcsEvent>, ctx: DcsSyncCtx) {
    let mut mem = MergeMemory::default();
    let mut last_sent: Option<Instant> = None;
    let mut had_update = false;
    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(DcsEvent::Export(info)) => {
                had_update = true;
                handle_export(&ctx, *info, &mut mem, &mut last_sent);
            }
            Ok(DcsEvent::GameGui(g)) => handle_gamegui(&ctx, g),
            Ok(DcsEvent::LosResults(results)) => {
                if let Ok(mut clients) = ctx.shared.clients.write() {
                    for r in results {
                        if let Some(c) = clients.get_mut(&r.id) {
                            c.local.line_of_sight_loss = r.los;
                        }
                    }
                }
            }
            Ok(DcsEvent::AutoConnect(addr)) => {
                let _ = ctx.app_events.send(AppEvent::AutoConnect(addr));
                (ctx.wake)();
            }
            Ok(DcsEvent::Command(cmd)) => {
                if handle_command(&ctx, &cmd) {
                    ctx.radio_state.lock().force_sync = true;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        // `CheckIfRadioIsStale` (1 s timer): no export for 10 s → reset.
        {
            let mut rs = ctx.radio_state.lock();
            if !rs.eam_active && had_update && !rs.dcs.is_current() && rs.dcs.last_update.is_some() {
                rs.side.position = LatLng::default();
                rs.dcs.reset();
                rs.dcs.last_update = None;
                last_sent = None;
                had_update = false;
                log::info!("Reset Radio state - no longer connected");
            }
        }
    }
    log::info!("DCS sync thread stopped");
}

/// `ProcessRadioInfoAsync` (+ the `LastSeenName` update of the listener).
fn handle_export(
    ctx: &DcsSyncCtx,
    message: srs_dcs::DcsPlayerRadioInfo,
    mem: &mut MergeMemory,
    last_sent: &mut Option<Instant>,
) {
    if !message.name.trim().is_empty() && message.name != "Unknown" {
        let mut st = ctx.state.lock();
        if st.last_seen_name != message.name {
            st.last_seen_name = message.name.clone();
            let _ = ctx
                .app_events
                .send(AppEvent::LastSeenNameChanged(message.name.clone()));
        }
    }
    let mut rs = ctx.radio_state.lock();
    if rs.eam_active {
        return; // DCS is ignored while in External AWACS Mode.
    }
    rs.dcs_export_last_received = Some(Instant::now());
    let mut mctx = ctx.merge_ctx.lock().clone();
    mctx.sending_on = if rs.sending.is_sending {
        rs.sending.sending_on
    } else {
        None
    };
    mctx.now = Instant::now();
    let outcome = merge(&mut rs.dcs, message, &mctx, mem);
    for e in &outcome.events {
        match e {
            MergeEvent::PositionUpdated(p) => rs.side.position = *p,
            MergeEvent::NewUnitEntered { unit, seat } => {
                let _ = ctx.app_events.send(AppEvent::NewUnitEntered {
                    unit: unit.clone(),
                    seat: *seat,
                });
            }
            MergeEvent::ReloadFixedChannels {
                radio,
                min,
                max,
                preselected,
            } => {
                // `PresetChannelsViewModel.Reload`: `.txt` presets + server presets.
                let list = match rs.dcs.radios.get(*radio) {
                    Some(r) => {
                        let client = srs_dcs::presets::load_from_store(
                            &mctx.presets_folder,
                            &r.name,
                            r.modulation == srs_protocol::Modulation::Mids,
                        );
                        let server = ctx
                            .shared
                            .server_settings
                            .read()
                            .map(|s| s.preset_channels(&srs_dcs::presets::normalise(&r.name)).to_vec())
                            .unwrap_or_default();
                        srs_dcs::presets::build_channel_list(
                            r.modulation,
                            *min,
                            *max,
                            client,
                            &server,
                            mctx.preset_source,
                        )
                    }
                    None => Vec::new(),
                };
                if let Some(slot) = ctx.fixed_channels.lock().get_mut(*radio) {
                    *slot = list.clone();
                }
                let _ = ctx.app_events.send(AppEvent::ReloadFixedChannels {
                    radio: *radio,
                    min: *min,
                    max: *max,
                });
                if mctx.auto_select_preset_channel {
                    rh::channel_up(&mut rs.dcs, *radio, &list);
                } else if mctx.eam_connected && *preselected != -1 {
                    if let Some(p) = list.get((*preselected - 1).max(0) as usize) {
                        rh::select_channel(&mut rs.dcs, *radio, p);
                    }
                }
            }
            MergeEvent::ClearFixedChannels { radio } => {
                if let Some(l) = ctx.fixed_channels.lock().get_mut(*radio) {
                    l.clear();
                }
            }
        }
    }
    // Overlay / panels state (7080, 7082).
    let my_guid = ctx.state.lock().guid.to_string();
    let line = build_line(&ctx.shared, &rs, &my_guid, true);
    ctx.dcs_io.send_state(line);

    let force = std::mem::take(&mut rs.force_sync);
    let stale = last_sent.is_none_or(|t| t.elapsed() > RADIO_UPDATE_PING_INTERVAL);
    if outcome.changed || force || stale {
        log::debug!(
            "Sending Radio Info To Server - Update (changed={} force={force} stale={stale})",
            outcome.changed
        );
        *last_sent = Some(Instant::now());
        let client = build_client(&rs, &ctx.state.lock());
        (ctx.net)(NetCommand::SendRadioUpdate(client));
    }
    (ctx.wake)();
}

/// `DCSGameGuiHandler` receive path.
fn handle_gamegui(ctx: &DcsSyncCtx, g: crate::dcs_io::GameGuiInfo) {
    let mut rs = ctx.radio_state.lock();
    rs.dcs_gamegui_last_received = Some(Instant::now());
    rs.side.name = g.name.clone();
    rs.side.side = g.side;
    rs.side.seat = g.seat;
    {
        let mut st = ctx.state.lock();
        st.last_seen_name = g.name.clone();
        st.coalition = g.side;
        st.seat = g.seat;
    }
    // `ClearPositionsIfExpired`: no export recently → drop the position.
    if !rs
        .dcs_export_last_received
        .is_some_and(|t| t.elapsed() < Duration::from_secs(10))
    {
        rs.side.position = LatLng::default();
    }
    let client = build_client(&rs, &ctx.state.lock());
    (ctx.net)(NetCommand::SendMetadata(client));
    (ctx.wake)();
}

/// `UDPCommandHandler`: returns true when the radio state must be re-sent.
fn handle_command(ctx: &DcsSyncCtx, cmd: &crate::dcs_io::UdpCommand) -> bool {
    let Some(kind) = cmd.kind() else {
        log::error!("Unknown UDP Command!");
        return false;
    };
    let radio = usize::try_from(cmd.radio_id).unwrap_or(usize::MAX);
    let rotary = ctx.rotary_style.load(Ordering::Relaxed);
    let mut rs = ctx.radio_state.lock();
    let info = &mut rs.dcs;
    match kind {
        UdpCommandType::FrequencyDelta => {
            rh::update_frequency(info, cmd.frequency, radio, true, true, rotary).0
        }
        UdpCommandType::FrequencySet => {
            rh::update_frequency(info, cmd.frequency, radio, false, true, rotary).0
        }
        UdpCommandType::ActiveRadio => {
            rh::select_radio(info, radio);
            false
        }
        UdpCommandType::ToggleGuard => rh::toggle_guard(info, radio),
        UdpCommandType::Guard => rh::set_guard(info, radio, cmd.enabled),
        UdpCommandType::ChannelUp => {
            let list = ctx.fixed_channels.lock().get(radio).cloned().unwrap_or_default();
            rh::channel_up(info, radio, &list)
        }
        UdpCommandType::ChannelDown => {
            let list = ctx.fixed_channels.lock().get(radio).cloned().unwrap_or_default();
            rh::channel_down(info, radio, &list)
        }
        UdpCommandType::SetVolume => {
            rh::set_volume(info, radio, cmd.volume);
            false
        }
        UdpCommandType::ToggleRadioRetransmit => rh::toggle_retransmit(info, radio),
        UdpCommandType::RadioRetransmit => rh::set_retransmit(info, radio, cmd.enabled),
        UdpCommandType::ToggleGlobalSimultaneousTransmission => {
            rh::toggle_global_simultaneous_transmission(info);
            false
        }
        UdpCommandType::GlobalSimultaneousTransmission => {
            rh::set_global_simultaneous_transmission(info, cmd.enabled);
            false
        }
        UdpCommandType::ToggleSimultaneousRadio => rh::toggle_simultaneous(info, radio),
        UdpCommandType::SimultaneousRadio => rh::set_simultaneous(info, radio, cmd.enabled),
        UdpCommandType::IntercomChannel => {
            let chan = u32::try_from(cmd.radio_id).unwrap_or(1);
            match rh::set_intercom(info, chan) {
                Some(c) => {
                    rs.intercom_offset = c;
                    true
                }
                None => false,
            }
        }
        // `TransponderHelper` always resets `LastSent` (forces a RADIO_UPDATE).
        UdpCommandType::TransponderPower => {
            srs_dcs::transponder::set_power(info, cmd.enabled);
            true
        }
        UdpCommandType::TransponderM1Code => {
            srs_dcs::transponder::set_mode1(info, cmd.code);
            true
        }
        UdpCommandType::TransponderM2Code => {
            srs_dcs::transponder::set_mode2(info, cmd.code);
            true
        }
        UdpCommandType::TransponderM3Code => {
            srs_dcs::transponder::set_mode3(info, cmd.code);
            true
        }
        UdpCommandType::TransponderM4 => {
            srs_dcs::transponder::set_mode4(info, cmd.enabled);
            true
        }
        UdpCommandType::TransponderIdent => {
            srs_dcs::transponder::set_ident(info, cmd.enabled);
            true
        }
    }
}
