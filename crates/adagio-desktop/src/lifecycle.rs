use crate::state::AppState;
use adagio_nextcloud::client::NextcloudClient;
use std::sync::Arc;
use tracing::{info, warn};

/// Spawn a `PairRunner` for every pair loaded from config.
///
/// For each pair, credentials are retrieved from the OS keychain via
/// `spawn_blocking`. Pairs with missing credentials are skipped with a warning;
/// the remaining pairs continue to start normally.
pub async fn start_engine_for_all_pairs(state: &AppState) {
    let pairs = {
        let mgr = state.pairs.read().await;
        mgr.all_pairs().into_iter().cloned().collect::<Vec<_>>()
    };

    let accounts = match state.accounts.list() {
        Ok(a) => a,
        Err(e) => {
            warn!(error = %e, "failed to list accounts for engine startup");
            return;
        }
    };

    for pair in pairs {
        let account = match accounts.iter().find(|a| a.id == pair.account_id) {
            Some(a) => a.clone(),
            None => {
                warn!(
                    pair_id = %pair.id,
                    account_id = %pair.account_id,
                    "account not found for pair; skipping"
                );
                continue;
            }
        };

        let key = account.keychain_service_key.clone();
        let password = match tokio::task::spawn_blocking(move || {
            adagio_nextcloud::auth::retrieve_credentials(&key)
        })
        .await
        {
            Ok(Ok(Some(pw))) => pw,
            Ok(Ok(None)) => {
                warn!(
                    pair_id = %pair.id,
                    account_id = %account.id,
                    "no credentials in keychain; skipping pair"
                );
                continue;
            }
            Ok(Err(e)) => {
                warn!(
                    pair_id = %pair.id,
                    error = %e,
                    "keychain error retrieving credentials; skipping pair"
                );
                continue;
            }
            Err(e) => {
                warn!(
                    pair_id = %pair.id,
                    error = %e,
                    "spawn_blocking panicked retrieving credentials; skipping pair"
                );
                continue;
            }
        };

        let client = Arc::new(NextcloudClient::new(
            &account.server_url,
            &account.username,
            &password,
        ));

        if let Err(e) = state.journal.register_pair(&account, &pair).await {
            warn!(pair_id = %pair.id, error = %e, "failed to register pair in journal; skipping");
            continue;
        }

        info!(
            pair_id = %pair.id,
            account_id = %account.id,
            "starting pair runner"
        );

        state
            .engine
            .start_pair(pair, client, state.journal.clone())
            .await;
    }
}
