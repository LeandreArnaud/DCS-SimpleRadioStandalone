//! Setting keys. **The enum variant names are the keys written to disk**
//! (`GlobalSettingsKeys`, `ProfileSettingsKeys`, `InputBinding` in C#).
//! Never rename; append new ones at the end.

macro_rules! string_enum {
    ($(#[$meta:meta])* $vis:vis enum $name:ident { $( $variant:ident ),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[allow(non_camel_case_types)]
        $vis enum $name { $( $variant ),* }

        impl $name {
            pub const ALL: &'static [$name] = &[ $( $name::$variant ),* ];

            /// The key as written in the `.cfg` file.
            pub const fn as_str(self) -> &'static str {
                match self { $( $name::$variant => stringify!($variant) ),* }
            }

            pub fn from_str_opt(s: &str) -> Option<Self> {
                match s { $( stringify!($variant) => Some($name::$variant), )* _ => None }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

string_enum! {
    /// `GlobalSettingsKeys` (global.cfg).
    pub enum GlobalSettingsKey {
        Version,
        MinimiseToTray,
        StartMinimised,
        RefocusDCS,
        ExpandControls,
        AutoConnectPrompt,
        RadioOverlayTaskbarHide,
        AudioInputDeviceId,
        AudioOutputDeviceId,
        LastServer,
        MicBoost,
        SpeakerBoost,
        RadioX,
        RadioY,
        RadioSize,
        RadioOpacity,
        RadioWidth,
        RadioHeight,
        ClientX,
        ClientY,
        AwacsX,
        AwacsY,
        MicAudioOutputDeviceId,
        CliendIdShort,
        ClientIdLong,
        DCSLOSOutgoingUDP,
        DCSIncomingUDP,
        CommandListenerUDP,
        OutgoingDCSUDPInfo,
        OutgoingDCSUDPOther,
        DCSIncomingGameGUIUDP,
        DCSLOSIncomingUDP,
        Denoise,
        DenoiseAttenuation,
        IncomingAudioAGC,
        IncomingAudioAGCTarget,
        IncomingAudioAGCDecrement,
        IncomingAudioAGCLevelMax,
        IncomingAudioDenoise,
        IncomingAudioDenoiseAttenuation,
        LastSeenName,
        CheckForBetaUpdates,
        AllowMultipleInstances,
        AutoConnectMismatchPrompt,
        DisableWindowVisibilityCheck,
        PlayConnectionSounds,
        RequireAdmin,
        LotATCIncomingUDP,
        LotATCOutgoingUDP,
        SettingsProfiles,
        AutoSelectSettingsProfile,
        VAICOMIncomingUDP,
        VAICOMTXInhibitEnabled,
        LotATCHeightOffset,
        DCSAutoConnectUDP,
        ShowTransmitterName,
        IdleTimeOut,
        AutoConnect,
        AllowRecording,
        RecordAudio,
        SingleFileMixdown,
        RecordingQuality,
        RecordingFormat,
        DisallowedAudioTone,
        VOX,
        VOXMode,
        VOXMinimumTime,
        VOXMinimumDB,
        AllowXInputController,
        LastPresetsFolder,
        DISEntityID,
        DarkMode,
    }
}

impl GlobalSettingsKey {
    /// Section the C# code reads/writes this key from. Position keys live in
    /// `[Position Settings]`, ports in `[Network Settings]`, the rest in
    /// `[Client Settings]`.
    pub const fn section(self) -> &'static str {
        use GlobalSettingsKey::*;
        match self {
            RadioX | RadioY | RadioSize | RadioOpacity | RadioWidth | RadioHeight | ClientX | ClientY
            | AwacsX | AwacsY => "Position Settings",
            DCSLOSOutgoingUDP
            | DCSIncomingUDP
            | CommandListenerUDP
            | OutgoingDCSUDPInfo
            | OutgoingDCSUDPOther
            | DCSIncomingGameGUIUDP
            | DCSLOSIncomingUDP
            | LotATCIncomingUDP
            | LotATCOutgoingUDP
            | VAICOMIncomingUDP
            | DCSAutoConnectUDP => "Network Settings",
            _ => "Client Settings",
        }
    }

    /// `defaultGlobalSettings`. `None` for keys without a default (the C#
    /// code then creates them with an empty value). `ClientIdLong` defaults
    /// to a fresh GUID in C#; we return `None` and let the store generate it.
    pub const fn default_value(self) -> Option<&'static str> {
        use GlobalSettingsKey::*;
        Some(match self {
            Version => "0",
            AutoConnect => "true",
            AutoConnectPrompt => "false",
            AutoConnectMismatchPrompt => "true",
            RadioOverlayTaskbarHide => "false",
            RefocusDCS => "false",
            ExpandControls => "false",
            MinimiseToTray => "false",
            StartMinimised => "false",
            AudioInputDeviceId => "",
            AudioOutputDeviceId => "",
            MicAudioOutputDeviceId => "",
            LastServer => "127.0.0.1",
            MicBoost => "0.514",
            SpeakerBoost => "0.514",
            RadioX => "300",
            RadioY => "300",
            RadioSize => "1.0",
            RadioOpacity => "1.0",
            RadioWidth => "122",
            RadioHeight => "270",
            ClientX => "200",
            ClientY => "200",
            AwacsX => "300",
            AwacsY => "300",
            DCSLOSOutgoingUDP => "9086",
            DCSIncomingUDP => "9084",
            CommandListenerUDP => "9040",
            OutgoingDCSUDPInfo => "7080",
            OutgoingDCSUDPOther => "7082",
            DCSIncomingGameGUIUDP => "5068",
            DCSLOSIncomingUDP => "9085",
            DCSAutoConnectUDP => "5069",
            Denoise => "true",
            DenoiseAttenuation => "-30",
            IncomingAudioAGC => "true",
            IncomingAudioAGCTarget => "14000",
            IncomingAudioAGCDecrement => "-60",
            IncomingAudioAGCLevelMax => "40",
            IncomingAudioDenoise => "true",
            IncomingAudioDenoiseAttenuation => "-30",
            LastSeenName => "",
            CheckForBetaUpdates => "false",
            AllowMultipleInstances => "false",
            DisableWindowVisibilityCheck => "false",
            PlayConnectionSounds => "true",
            RequireAdmin => "false",
            AutoSelectSettingsProfile => "false",
            LotATCIncomingUDP => "10710",
            LotATCOutgoingUDP => "10711",
            LotATCHeightOffset => "50",
            VAICOMIncomingUDP => "33501",
            VAICOMTXInhibitEnabled => "false",
            ShowTransmitterName => "true",
            IdleTimeOut => "600",
            AllowRecording => "false",
            RecordAudio => "false",
            SingleFileMixdown => "false",
            RecordingQuality => "V3",
            RecordingFormat => "mp3",
            DisallowedAudioTone => "false",
            VOX => "false",
            VOXMode => "3",
            VOXMinimumTime => "300",
            VOXMinimumDB => "-59.0",
            AllowXInputController => "false",
            LastPresetsFolder => "",
            DISEntityID => "-1",
            DarkMode => "false",
            CliendIdShort | ClientIdLong | SettingsProfiles => return None,
        })
    }
}

string_enum! {
    /// `ProfileSettingsKeys` (`[Client Settings]` of a profile file).
    pub enum ProfileSettingsKey {
        Radio1Channel,
        Radio2Channel,
        Radio3Channel,
        Radio4Channel,
        Radio5Channel,
        Radio6Channel,
        Radio7Channel,
        Radio8Channel,
        Radio9Channel,
        Radio10Channel,
        IntercomChannel,
        RadioEffectsRatio,
        RadioEncryptionEffects,
        RadioEffectsClipping,
        NATOTone,
        RadioRxEffects_Start,
        RadioRxEffects_End,
        RadioTxEffects_Start,
        RadioTxEffects_End,
        AutoSelectPresetChannel,
        AlwaysAllowHotasControls,
        AllowDCSPTT,
        RadioSwitchIsPTT,
        AlwaysAllowTransponderOverlay,
        RadioSwitchIsPTTOnlyWhenValid,
        MIDSRadioEffect,
        PTTReleaseDelay,
        RadioTransmissionStartSelection,
        RadioTransmissionEndSelection,
        HAVEQUICKTone,
        RadioBackgroundNoiseEffect,
        NATOToneVolume,
        HQToneVolume,
        NoiseGainDB,
        HFNoiseGainDB,
        PerRadioModelEffects,
        PTTStartDelay,
        RotaryStyleIncrement,
        IntercomTransmissionStartSelection,
        IntercomTransmissionEndSelection,
        AMCollisionVolume,
        AmbientCockpitNoiseEffect,
        AmbientCockpitNoiseEffectVolume,
        AmbientCockpitIntercomNoiseEffect,
        DisableExpansionRadios,
        ServerPresetSelection,
        AllowServerEAMRadioPreset,
        InstructorMode,
    }
}

impl ProfileSettingsKey {
    /// `DefaultSettingsProfileSettings`. Keys without a default are created
    /// with an empty value by the C# code.
    pub const fn default_value(self) -> Option<&'static str> {
        use ProfileSettingsKey::*;
        Some(match self {
            RadioEffectsRatio => "1.0",
            RadioEffectsClipping => "false",
            RadioEncryptionEffects => "true",
            NATOTone => "true",
            HAVEQUICKTone => "true",
            RadioRxEffects_Start => "true",
            RadioRxEffects_End => "true",
            RadioTransmissionStartSelection => "RADIO_TRANS_START.wav",
            RadioTransmissionEndSelection => "RADIO_TRANS_END.wav",
            RadioTxEffects_Start => "true",
            RadioTxEffects_End => "true",
            MIDSRadioEffect => "true",
            AutoSelectPresetChannel => "true",
            AlwaysAllowHotasControls => "false",
            AllowDCSPTT => "true",
            RadioSwitchIsPTT => "false",
            RadioSwitchIsPTTOnlyWhenValid => "false",
            AlwaysAllowTransponderOverlay => "false",
            PTTReleaseDelay => "0",
            PTTStartDelay => "0",
            RadioBackgroundNoiseEffect => "true",
            NATOToneVolume => "1.2",
            HQToneVolume => "0.3",
            NoiseGainDB => "0",
            HFNoiseGainDB => "0",
            PerRadioModelEffects => "true",
            AMCollisionVolume => "1.0",
            RotaryStyleIncrement => "false",
            AmbientCockpitNoiseEffect => "true",
            AmbientCockpitNoiseEffectVolume => "1.0",
            AmbientCockpitIntercomNoiseEffect => "false",
            DisableExpansionRadios => "false",
            ServerPresetSelection => "USE_CLIENT_AND_SERVER_IF_SET",
            AllowServerEAMRadioPreset => "true",
            InstructorMode => "false",
            Radio1Channel | Radio2Channel | Radio3Channel | Radio4Channel | Radio5Channel | Radio6Channel
            | Radio7Channel | Radio8Channel | Radio9Channel | Radio10Channel | IntercomChannel => "0",
            IntercomTransmissionStartSelection | IntercomTransmissionEndSelection => return None,
        })
    }
}

/// `ServerPresetConfiguration` values of `ServerPresetSelection`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ServerPresetSelection {
    UseServerOnlyIfSet,
    UseClientOnly,
    #[default]
    UseClientAndServerIfSet,
}

impl ServerPresetSelection {
    pub const fn as_str(self) -> &'static str {
        match self {
            ServerPresetSelection::UseServerOnlyIfSet => "USE_SERVER_ONLY_IF_SET",
            ServerPresetSelection::UseClientOnly => "USE_CLIENT_ONLY",
            ServerPresetSelection::UseClientAndServerIfSet => "USE_CLIENT_AND_SERVER_IF_SET",
        }
    }

    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "USE_SERVER_ONLY_IF_SET" => Some(Self::UseServerOnlyIfSet),
            "USE_CLIENT_ONLY" => Some(Self::UseClientOnly),
            "USE_CLIENT_AND_SERVER_IF_SET" => Some(Self::UseClientAndServerIfSet),
            _ => None,
        }
    }
}

macro_rules! input_bindings {
    ($( $variant:ident = $num:literal ),* $(,)?) => {
        /// `InputBinding`. The numeric value is what the C# code iterates over
        /// (100..=137, modifier = value + 100). The name is the `.cfg` section.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(i32)]
        pub enum InputBinding { $( $variant = $num ),* }

        impl InputBinding {
            pub const ALL: &'static [InputBinding] = &[ $( InputBinding::$variant ),* ];

            pub const fn as_str(self) -> &'static str {
                match self { $( InputBinding::$variant => stringify!($variant) ),* }
            }

            pub fn from_str_opt(s: &str) -> Option<Self> {
                match s { $( stringify!($variant) => Some(InputBinding::$variant), )* _ => None }
            }

            pub const fn from_i32(v: i32) -> Option<Self> {
                match v { $( $num => Some(InputBinding::$variant), )* _ => None }
            }
        }
    };
}

input_bindings! {
    Intercom = 100,
    ModifierIntercom = 200,
    Switch1 = 101,
    ModifierSwitch1 = 201,
    Switch2 = 102,
    ModifierSwitch2 = 202,
    Switch3 = 103,
    ModifierSwitch3 = 203,
    Switch4 = 104,
    ModifierSwitch4 = 204,
    Switch5 = 105,
    ModifierSwitch5 = 205,
    Switch6 = 106,
    ModifierSwitch6 = 206,
    Switch7 = 107,
    ModifierSwitch7 = 207,
    Switch8 = 108,
    ModifierSwitch8 = 208,
    Switch9 = 109,
    ModifierSwitch9 = 209,
    Switch10 = 110,
    ModifierSwitch10 = 210,
    Ptt = 111,
    ModifierPtt = 211,
    OverlayToggle = 112,
    ModifierOverlayToggle = 212,
    Up100 = 113,
    ModifierUp100 = 213,
    Up10 = 114,
    ModifierUp10 = 214,
    Up1 = 115,
    ModifierUp1 = 215,
    Up01 = 116,
    ModifierUp01 = 216,
    Up001 = 117,
    ModifierUp001 = 217,
    Up0001 = 118,
    ModifierUp0001 = 218,
    Down100 = 119,
    ModifierDown100 = 219,
    Down10 = 120,
    ModifierDown10 = 220,
    Down1 = 121,
    ModifierDown1 = 221,
    Down01 = 122,
    ModifierDown01 = 222,
    Down001 = 123,
    ModifierDown001 = 223,
    Down0001 = 124,
    ModifierDown0001 = 224,
    NextRadio = 125,
    ModifierNextRadio = 225,
    PreviousRadio = 126,
    ModifierPreviousRadio = 226,
    ToggleGuard = 127,
    ModifierToggleGuard = 227,
    ToggleEncryption = 128,
    ModifierToggleEncryption = 228,
    EncryptionKeyIncrease = 129,
    ModifierEncryptionKeyIncrease = 229,
    EncryptionKeyDecrease = 130,
    ModifierEncryptionEncryptionKeyDecrease = 230,
    RadioChannelUp = 131,
    ModifierRadioChannelUp = 231,
    RadioChannelDown = 132,
    ModifierRadioChannelDown = 232,
    TransponderIDENT = 133,
    ModifierTransponderIDENT = 233,
    RadioVolumeUp = 134,
    ModifierRadioVolumeUp = 234,
    RadioVolumeDown = 135,
    ModifierRadioVolumeDown = 235,
    IntercomPTT = 136,
    ModifierIntercomPTT = 236,
    AwacsOverlayToggle = 137,
    ModifierAwacsOverlayToggle = 237,
    TransponderPowerToggle = 138,
    ModifierTransponderPowerToggle = 238,
}

impl InputBinding {
    pub const fn value(self) -> i32 {
        self as i32
    }

    pub const fn is_modifier(self) -> bool {
        self.value() >= 200
    }

    /// The modifier binding paired with this action (`bind + 100`).
    pub const fn modifier(self) -> Option<InputBinding> {
        if self.is_modifier() {
            None
        } else {
            InputBinding::from_i32(self.value() + 100)
        }
    }

    /// Non-modifier bindings in the order the C# poll loop visits them
    /// (`Intercom` .. `AwacsOverlayToggle`; `TransponderPowerToggle` is never
    /// polled by the C# client, see `pieges-et-dette.md`).
    pub fn polled() -> impl Iterator<Item = InputBinding> {
        (100..=137).filter_map(InputBinding::from_i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_values() {
        assert_eq!(GlobalSettingsKey::DCSIncomingUDP.as_str(), "DCSIncomingUDP");
        assert_eq!(GlobalSettingsKey::DCSIncomingUDP.section(), "Network Settings");
        assert_eq!(GlobalSettingsKey::RadioX.section(), "Position Settings");
        assert_eq!(
            GlobalSettingsKey::from_str_opt("DarkMode"),
            Some(GlobalSettingsKey::DarkMode)
        );
        assert_eq!(GlobalSettingsKey::ALL.len(), 72);
        assert_eq!(ProfileSettingsKey::ALL.len(), 48);
        assert_eq!(InputBinding::ALL.len(), 78);
        assert_eq!(InputBinding::Ptt.value(), 111);
        assert_eq!(InputBinding::Ptt.modifier(), Some(InputBinding::ModifierPtt));
        assert_eq!(InputBinding::ModifierPtt.as_str(), "ModifierPtt");
        assert_eq!(InputBinding::polled().count(), 38);
        assert_eq!(
            InputBinding::polled().last(),
            Some(InputBinding::AwacsOverlayToggle)
        );
    }
}
