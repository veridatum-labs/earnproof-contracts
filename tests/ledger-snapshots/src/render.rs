//! The normalization boundary.
//!
//! Everything a snapshot contains passes through this module, and nothing
//! bypasses it. Two rules govern what it does.
//!
//! **It excludes host metadata.** Host object handles, live-until ledgers,
//! entry sizes, and budget counters are properties of the environment a call
//! ran in, not of the state the contract produced. They move for reasons that
//! have nothing to do with a compatibility break, so a snapshot that included
//! them would be rewritten constantly and would stop being read. Values are
//! rendered from `ScVal`, the serialized form, which carries none of that.
//!
//! **It hides no contract state.** Every field of every stored record is
//! rendered, in full, including fields a test happens not to care about. The
//! only substitution is the address alias table below, which replaces a
//! generated address with the role it plays, and that substitution is
//! bidirectional and total: an address with no alias renders as an explicit
//! failure marker rather than passing through.

use soroban_sdk::xdr::{ContractEvent, ContractEventBody, ContractId, ScAddress, ScVal};
use soroban_sdk::{Address, Env, TryFromVal, Val};

/// Maps generated addresses to the role they play in a scenario.
///
/// Fixtures never contain an address. Test-generated addresses are stable but
/// meaningless, and a fixture full of `CAAAA...` is unreviewable; more
/// importantly, an alias makes it obvious at review time that no fixture ever
/// carries a real deployment address or a production identifier.
pub struct Aliases {
    entries: std::vec::Vec<(std::string::String, std::string::String)>,
}

impl Aliases {
    pub fn new() -> Self {
        Self {
            entries: std::vec::Vec::new(),
        }
    }

    pub fn insert(&mut self, address: &Address, alias: &str) {
        self.entries.push((
            key_for(&ScAddress::from(address)),
            std::string::String::from(alias),
        ));
    }

    fn lookup(&self, rendered: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(address, _)| address == rendered)
            .map(|(_, alias)| alias.as_str())
    }
}

impl Default for Aliases {
    fn default() -> Self {
        Self::new()
    }
}

/// Renders a host value as normalized text.
pub fn value(env: &Env, aliases: &Aliases, val: &Val) -> std::string::String {
    let scval = ScVal::try_from_val(env, val).expect("value is not representable as ScVal");
    scval_text(aliases, &scval)
}

/// Renders one emitted event from its XDR form.
///
/// Events reach this renderer already serialized, so there is no host metadata
/// to strip: the contract id becomes an alias, and the topics and data are
/// rendered exactly as an indexer would receive them.
pub fn event(aliases: &Aliases, event: &ContractEvent) -> std::string::String {
    let contract = match &event.contract_id {
        Some(hash) => contract_alias(aliases, hash),
        None => std::string::String::from("<no-contract>"),
    };
    let ContractEventBody::V0(body) = &event.body;
    let topics: std::vec::Vec<std::string::String> = body
        .topics
        .iter()
        .map(|topic| scval_text(aliases, topic))
        .collect();
    std::format!(
        "{contract} topics=[{}] data={}",
        topics.join(", "),
        scval_text(aliases, &body.data)
    )
}

fn contract_alias(aliases: &Aliases, id: &ContractId) -> std::string::String {
    let address = ScAddress::Contract(id.clone());
    match aliases.lookup(&key_for(&address)) {
        Some(alias) => std::string::String::from(alias),
        None => std::string::String::from("<UNALIASED>"),
    }
}

fn scval_text(aliases: &Aliases, scval: &ScVal) -> std::string::String {
    match scval {
        ScVal::Void => "void".into(),
        ScVal::Bool(value) => std::format!("{value}"),
        ScVal::U32(value) => std::format!("u32:{value}"),
        ScVal::I32(value) => std::format!("i32:{value}"),
        ScVal::U64(value) => std::format!("u64:{value}"),
        ScVal::I64(value) => std::format!("i64:{value}"),
        ScVal::Symbol(symbol) => std::format!(
            "sym:{}",
            std::str::from_utf8(symbol.0.as_slice()).unwrap_or("<non-utf8>")
        ),
        ScVal::Bytes(bytes) => std::format!("bytes:{}", hex(bytes.0.as_slice())),
        ScVal::Address(address) => address_text(aliases, address),
        ScVal::Vec(Some(items)) => {
            let rendered: std::vec::Vec<std::string::String> = items
                .0
                .iter()
                .map(|item| scval_text(aliases, item))
                .collect();
            std::format!("[{}]", rendered.join(", "))
        }
        ScVal::Map(Some(entries)) => {
            // ScMap is canonically ordered by key, so this rendering is stable
            // without any sorting of our own.
            let rendered: std::vec::Vec<std::string::String> = entries
                .0
                .iter()
                .map(|entry| {
                    std::format!(
                        "{}: {}",
                        scval_text(aliases, &entry.key),
                        scval_text(aliases, &entry.val)
                    )
                })
                .collect();
            std::format!("{{{}}}", rendered.join(", "))
        }
        // Anything not listed above is state this renderer would silently drop,
        // which is exactly what the module must never do.
        other => std::format!("<unrendered:{other:?}>"),
    }
}

/// Opaque, stable identity for an address. Never written to a fixture; used
/// only to look an alias up.
fn key_for(address: &ScAddress) -> std::string::String {
    std::format!("{address:?}")
}

fn address_text(aliases: &Aliases, address: &ScAddress) -> std::string::String {
    let rendered = key_for(address);
    match aliases.lookup(&rendered) {
        Some(alias) => std::format!("addr:{alias}"),
        // Loud on purpose. A snapshot must never carry an unaliased address,
        // so an unknown one is recorded as a failure rather than written out.
        None => std::string::String::from("addr:<UNALIASED>"),
    }
}

fn hex(bytes: &[u8]) -> std::string::String {
    let mut out = std::string::String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&std::format!("{byte:02x}"));
    }
    out
}
