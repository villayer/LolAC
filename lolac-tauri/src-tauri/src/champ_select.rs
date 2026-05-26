// TODO: Implement auto-pick and auto-ban during champion select
// LCU endpoints to use when implementing:
//   GET  /lol-champ-select/v1/session        -- current session state
//   POST /lol-champ-select/v1/session/actions/{id}/complete  -- lock in pick
//   PATCH /lol-champ-select/v1/session/actions/{id}          -- set champion
// Subscribe to: OnJsonApiEvent_lol-champ-select_v1_session

pub async fn auto_pick_stub() {
    println!("[champ-select] auto-pick not yet implemented");
}

pub async fn auto_ban_stub() {
    println!("[champ-select] auto-ban not yet implemented");
}
