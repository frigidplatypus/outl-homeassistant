//! "What each device still has to sync" (`outl_sync_iroh::pending_by_peer`).
//!
//! A confirmed exchange sets the floor; a local write after it is pending
//! until the next one; a peer we never exchanged with is absent, not zero.

// `common` is shared by every integration test; this one uses a subset.
#[allow(dead_code)]
mod common;

use std::sync::mpsc;

use outl_core::id::ActorId;
use outl_sync_iroh::test_support;

use common::{fresh_identity, seed_ops, shared_wid, STEP_TIMEOUT};

/// Stand up a responder B and return everything an initiator A needs.
async fn paired_pair() -> (
    tempfile::TempDir,
    tempfile::TempDir,
    ActorId,
    iroh::Endpoint,
    iroh::EndpointAddr,
    iroh::protocol::Router,
) {
    let dir_a = tempfile::tempdir().expect("A tempdir");
    let dir_b = tempfile::tempdir().expect("B tempdir");
    let actor_a = ActorId::new();
    let actor_b = ActorId::new();

    let id_a = fresh_identity(dir_a.path(), "a");
    let id_b = fresh_identity(dir_b.path(), "b");

    let ep_a = test_support::bind_sync_endpoint(&id_a)
        .await
        .expect("bind A");
    let ep_b = test_support::bind_sync_endpoint(&id_b)
        .await
        .expect("bind B");
    let b_addr = ep_b.addr();

    let (b_ready_tx, _b_ready_rx) = mpsc::channel::<()>();
    let router_b = test_support::spawn_responder(
        ep_b,
        dir_b.path().to_path_buf(),
        shared_wid(),
        actor_b,
        b_ready_tx,
        &[id_a.node_id()],
    );

    (dir_a, dir_b, actor_a, ep_a, b_addr, router_b)
}

async fn sync(dir: &std::path::Path, actor: ActorId, ep: &iroh::Endpoint, to: &iroh::EndpointAddr) {
    let conns = test_support::connection_pool(ep.clone());
    let (tx, _rx) = mpsc::channel::<()>();
    tokio::time::timeout(
        STEP_TIMEOUT,
        test_support::run_delta_sync_pooled(&conns, to.clone(), dir, &shared_wid(), actor, tx),
    )
    .await
    .expect("sync did not finish in time")
    .expect("sync must succeed");
}

#[tokio::test(flavor = "multi_thread")]
async fn writes_after_a_sync_are_pending_until_the_next_one() {
    let (dir_a, _dir_b, actor_a, ep_a, b_addr, _router_b) = paired_pair().await;
    let b = b_addr.id.to_string();
    seed_ops(dir_a.path(), actor_a, 3);

    let before = outl_sync_iroh::pending_by_peer(dir_a.path(), actor_a).unwrap();
    assert!(
        !before.contains_key(&b),
        "no exchange yet, so no number to show"
    );

    sync(dir_a.path(), actor_a, &ep_a, &b_addr).await;
    let after = outl_sync_iroh::pending_by_peer(dir_a.path(), actor_a).unwrap();
    assert_eq!(after.get(&b), Some(&0), "B confirmed everything A pushed");

    seed_ops(dir_a.path(), actor_a, 2);
    let later = outl_sync_iroh::pending_by_peer(dir_a.path(), actor_a).unwrap();
    assert_eq!(later.get(&b), Some(&2));

    sync(dir_a.path(), actor_a, &ep_a, &b_addr).await;
    let again = outl_sync_iroh::pending_by_peer(dir_a.path(), actor_a).unwrap();
    assert_eq!(again.get(&b), Some(&0));
}
