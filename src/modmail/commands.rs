//! Commandes texte du staff (`!reply`, `!close`, `!logs`…), mêmes noms, alias,
//! signatures et réponses que modmailbot.

use super::{
    Modmail,
    models::{Thread, thread_status},
    parser::{self, Args, Kind, Param},
    store,
    thread::{Moderator, NewThreadOptions, StaffReply},
    util::{
        chunk_lines, chunk_message_lines, db_time_in, disable_code_blocks, escape_markdown, humanize_delay,
        humanize_duration, is_snowflake, parse_db_time,
    },
};
use crate::{
    context::Ctx,
    discord::messages::{Mentions, parse_id},
};
use anyhow::Result;
use twilight_model::{
    channel::{ChannelType, Message, permission_overwrite::PermissionOverwrite},
    id::{
        Id,
        marker::{ChannelMarker, RoleMarker, UserMarker},
    },
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    /// Partout, y compris en message privé.
    Global,
    /// Serveur inbox, membres du staff.
    InboxServer,
    /// Salon d'un ticket ouvert (ou suspendu si `allow_suspended`).
    InboxThread { allow_suspended: bool },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cmd {
    Reply,
    AnonReply,
    RealReply,
    Edit,
    Delete,
    Close,
    Logs,
    Log,
    Block,
    Unblock,
    IsBlocked,
    Blocklist,
    Move,
    Snippet,
    DeleteSnippet,
    EditSnippet,
    Snippets,
    SuspendCancel,
    Suspend,
    Unsuspend,
    PrivacyPolicyLink,
    Version,
    NewThread,
    Id,
    DmChannelId,
    MessageInfo,
    Alert,
    ThreadRole,
    ThreadRoleReset,
    ThreadRoleSet,
    DefaultRole,
    DefaultRoleReset,
    DefaultRoleSet,
    UserNotes,
    ThreadNotes,
    AddUserNote,
    AddThreadNote,
    DeleteNote,
}

struct Variant {
    trigger: &'static str,
    aliases: &'static [&'static str],
    scope: Scope,
    params: &'static [Param],
    switches: &'static [(&'static str, &'static str)],
    cmd: Cmd,
}

const THREAD: Scope = Scope::InboxThread { allow_suspended: false };
const THREAD_OR_SUSPENDED: Scope = Scope::InboxThread { allow_suspended: true };
const SERVER: Scope = Scope::InboxServer;
const LOG_SWITCHES: &[(&str, &str)] = &[("verbose", "v"), ("simple", "s")];

const fn v(
    trigger: &'static str,
    aliases: &'static [&'static str],
    scope: Scope,
    params: &'static [Param],
    cmd: Cmd,
) -> Variant {
    Variant { trigger, aliases, scope, params, switches: &[], cmd }
}

use Kind::{Delay, Number, Str, UserId};
use Param::{Optional, Required, Rest, Variadic};

/// Ordre d'enregistrement des modules de modmailbot : il départage les signatures.
fn variants(config: &super::config::MmConfig) -> Vec<Variant> {
    let mut list = vec![
        v("reply", &["r"], THREAD, &[Rest("text", false)], Cmd::Reply),
        v("anonreply", &["ar"], THREAD, &[Rest("text", false)], Cmd::AnonReply),
        v("realreply", &["rr"], THREAD, &[Rest("text", false)], Cmd::RealReply),
    ];
    if config.allow_staff_edit {
        list.push(v("edit", &["e"], THREAD, &[Required("messageNumber", Number), Rest("text", true)], Cmd::Edit));
    }
    if config.allow_staff_delete {
        list.push(v("delete", &["d"], THREAD, &[Required("messageNumber", Number)], Cmd::Delete));
    }
    list.push(Variant {
        trigger: "close",
        aliases: &[],
        scope: Scope::Global,
        params: &[Variadic("opts")],
        switches: &[("silent", "s"), ("cancel", "c")],
        cmd: Cmd::Close,
    });

    list.extend([
        Variant {
            trigger: "logs",
            aliases: &[],
            scope: SERVER,
            params: &[Required("userId", UserId), Optional("page", Number)],
            switches: LOG_SWITCHES,
            cmd: Cmd::Logs,
        },
        Variant {
            trigger: "logs",
            aliases: &[],
            scope: SERVER,
            params: &[Optional("page", Number)],
            switches: LOG_SWITCHES,
            cmd: Cmd::Logs,
        },
        Variant {
            trigger: "log",
            aliases: &["thread"],
            scope: THREAD_OR_SUSPENDED,
            params: &[],
            switches: LOG_SWITCHES,
            cmd: Cmd::Log,
        },
        Variant {
            trigger: "loglink",
            aliases: &[],
            scope: THREAD_OR_SUSPENDED,
            params: &[],
            switches: LOG_SWITCHES,
            cmd: Cmd::Log,
        },
        Variant {
            trigger: "log",
            aliases: &["thread"],
            scope: SERVER,
            params: &[Required("threadId", Str)],
            switches: LOG_SWITCHES,
            cmd: Cmd::Log,
        },
        Variant {
            trigger: "loglink",
            aliases: &[],
            scope: SERVER,
            params: &[Required("threadId", Str)],
            switches: LOG_SWITCHES,
            cmd: Cmd::Log,
        },
    ]);

    if config.allow_block {
        list.extend([
            v("block", &[], SERVER, &[Required("userId", UserId), Optional("blockTime", Delay)], Cmd::Block),
            v("block", &[], SERVER, &[Optional("blockTime", Delay)], Cmd::Block),
            v("unblock", &[], SERVER, &[Required("userId", UserId), Optional("unblockDelay", Delay)], Cmd::Unblock),
            v("unblock", &[], SERVER, &[Optional("unblockDelay", Delay)], Cmd::Unblock),
            v("is_blocked", &[], SERVER, &[Optional("userId", UserId)], Cmd::IsBlocked),
            v("blocklist", &[], SERVER, &[], Cmd::Blocklist),
        ]);
    }
    if config.allow_move {
        list.push(v("move", &[], THREAD, &[Rest("category", true)], Cmd::Move));
    }
    if config.allow_snippets {
        list.extend([
            v("snippet", &["s"], SERVER, &[Required("trigger", Str), Rest("text", false)], Cmd::Snippet),
            v("delete_snippet", &["ds"], SERVER, &[Required("trigger", Str)], Cmd::DeleteSnippet),
            v("edit_snippet", &["es"], SERVER, &[Required("trigger", Str), Rest("text", true)], Cmd::EditSnippet),
            v("snippets", &["s"], SERVER, &[], Cmd::Snippets),
        ]);
    }
    if config.allow_suspend {
        list.extend([
            v("suspend cancel", &[], THREAD, &[], Cmd::SuspendCancel),
            v("suspend", &[], THREAD_OR_SUSPENDED, &[Optional("delay", Delay)], Cmd::Suspend),
            v("unsuspend", &[], SERVER, &[], Cmd::Unsuspend),
        ]);
    }
    list.extend([
        v("privacy_policy_link", &[], SERVER, &[], Cmd::PrivacyPolicyLink),
        v("version", &[], SERVER, &[], Cmd::Version),
        v("newthread", &[], SERVER, &[Required("userId", UserId)], Cmd::NewThread),
        v("id", &[], THREAD_OR_SUSPENDED, &[], Cmd::Id),
        v("dm_channel_id", &[], THREAD_OR_SUSPENDED, &[], Cmd::DmChannelId),
        v("message", &[], THREAD_OR_SUSPENDED, &[Required("messageNumber", Number)], Cmd::MessageInfo),
        v("alert", &[], THREAD_OR_SUSPENDED, &[Optional("opt", Str)], Cmd::Alert),
    ]);
    if config.allow_changing_display_role {
        list.extend([
            v("role", &[], THREAD_OR_SUSPENDED, &[], Cmd::ThreadRole),
            v("role reset", &["role_reset", "reset_role"], THREAD_OR_SUSPENDED, &[], Cmd::ThreadRoleReset),
            v("role", &[], THREAD_OR_SUSPENDED, &[Rest("role", true)], Cmd::ThreadRoleSet),
            v("role", &[], SERVER, &[], Cmd::DefaultRole),
            v("role reset", &["role_reset", "reset_role"], SERVER, &[], Cmd::DefaultRoleReset),
            v("role", &[], SERVER, &[Rest("role", true)], Cmd::DefaultRoleSet),
        ]);
    }
    if config.allow_notes {
        list.extend([
            v("notes", &[], SERVER, &[Required("userId", UserId)], Cmd::UserNotes),
            v("notes", &[], THREAD, &[], Cmd::ThreadNotes),
            v("note", &[], SERVER, &[Required("userId", UserId), Rest("body", true)], Cmd::AddUserNote),
            v("note", &[], THREAD, &[Rest("body", true)], Cmd::AddThreadNote),
            v("delete_note", &["deletenote", "delnote"], SERVER, &[Required("noteId", Number)], Cmd::DeleteNote),
        ]);
    }
    list
}

/// Contexte d'exécution d'une commande.
struct Call<'a> {
    ctx: &'a Ctx,
    mm: &'a Modmail,
    msg: &'a Message,
    args: Args,
    thread: Option<Thread>,
}

impl Call<'_> {
    fn channel(&self) -> Id<ChannelMarker> {
        self.msg.channel_id
    }

    async fn say(&self, content: &str) -> Result<()> {
        self.say_with(content, Mentions::None).await
    }

    async fn say_with(&self, content: &str, mentions: Mentions) -> Result<()> {
        let allowed = mentions.to_allowed();
        self.ctx.http.create_message(self.channel()).content(content).allowed_mentions(Some(&allowed)).await?;
        Ok(())
    }

    async fn error(&self, content: &str) -> Result<()> {
        self.say(&format!("⚠ {content}")).await
    }

    /// `postSystemMessageWithFallback` : dans un ticket, message système ; sinon message simple.
    async fn system_or_say(&self, content: &str) -> Result<()> {
        match &self.thread {
            Some(thread) => self.mm.post_system_message(self.ctx, thread, content).await.map(drop),
            None => self.say(content).await,
        }
    }

    fn thread(&self) -> &Thread {
        self.thread.as_ref().expect("commande de ticket sans ticket")
    }

    fn moderator(&self) -> Moderator {
        Moderator {
            user: self.msg.author.clone(),
            nick: self.msg.member.as_ref().and_then(|m| m.nick.clone()),
            roles: self.msg.member.as_ref().map(|m| m.roles.clone()).unwrap_or_default(),
        }
    }

    fn author_name(&self) -> String {
        self.mm.display_name(&self.msg.author)
    }

    async fn delete_command_message(&self) {
        let _ = self.ctx.http.delete_message(self.msg.channel_id, self.msg.id).await;
    }

    /// Utilisateur visé : argument `userId`, sinon l'utilisateur du ticket.
    fn target_user(&self) -> Option<String> {
        self.args.str("userId").map(str::to_owned).or_else(|| self.thread.as_ref().map(|t| t.user_id.clone()))
    }
}

impl Modmail {
    /// Exécute la commande contenue dans le message, s'il y en a une.
    pub async fn handle_command(&self, ctx: &Ctx, msg: &Message) -> Result<()> {
        let Some(body) = msg.content.strip_prefix(self.config.prefix.as_str()) else {
            return Ok(());
        };
        if body.is_empty() || body.starts_with(char::is_whitespace) {
            return Ok(());
        }

        let on_inbox = msg.guild_id == Some(self.config.inbox_server_id);
        let is_staff = on_inbox && self.is_staff(ctx, msg);
        let mut first_error: Option<String> = None;

        for variant in variants(&self.config) {
            let triggers = std::iter::once(variant.trigger).chain(variant.aliases.iter().copied()).chain(
                self.config
                    .command_aliases
                    .iter()
                    .filter(|(_, target)| target.as_str() == variant.trigger)
                    .map(|(alias, _)| alias.as_str()),
            );
            let Some(rest) = triggers.into_iter().find_map(|trigger| parser::match_trigger(body, trigger)) else {
                continue;
            };

            // Pré-filtres de portée (serveur inbox, staff, ticket).
            let thread = match variant.scope {
                Scope::Global => None,
                Scope::InboxServer => {
                    if !is_staff {
                        continue;
                    }
                    store::find_by_channel_and_status(&self.db, &msg.channel_id.to_string(), thread_status::OPEN)
                        .await?
                }
                Scope::InboxThread { allow_suspended } => {
                    if !is_staff {
                        continue;
                    }
                    let found = if allow_suspended {
                        store::find_by_channel_id(&self.db, &msg.channel_id.to_string()).await?
                    } else {
                        store::find_by_channel_and_status(&self.db, &msg.channel_id.to_string(), thread_status::OPEN)
                            .await?
                    };
                    if found.is_none() {
                        continue;
                    }
                    found
                }
            };

            match parser::parse(rest, variant.params, variant.switches) {
                Ok(args) => {
                    let call = Call { ctx, mm: self, msg, args, thread };
                    return self.run(&call, variant.cmd).await;
                }
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }

        if let Some(error) = first_error {
            let _ = ctx.http.create_message(msg.channel_id).content(&format!("⚠ {error}")).await;
        }
        Ok(())
    }

    async fn run(&self, c: &Call<'_>, cmd: Cmd) -> Result<()> {
        match cmd {
            Cmd::Reply => self.cmd_reply(c, self.config.force_anon).await,
            Cmd::AnonReply => self.cmd_reply(c, true).await,
            Cmd::RealReply => self.cmd_reply(c, false).await,
            Cmd::Edit => self.cmd_edit(c).await,
            Cmd::Delete => self.cmd_delete(c).await,
            Cmd::Close => self.cmd_close(c).await,
            Cmd::Logs => self.cmd_logs(c).await,
            Cmd::Log => self.cmd_log(c).await,
            Cmd::Block => self.cmd_block(c).await,
            Cmd::Unblock => self.cmd_unblock(c).await,
            Cmd::IsBlocked => self.cmd_is_blocked(c).await,
            Cmd::Blocklist => self.cmd_blocklist(c).await,
            Cmd::Move => self.cmd_move(c).await,
            Cmd::Snippet => self.cmd_snippet(c).await,
            Cmd::DeleteSnippet => self.cmd_delete_snippet(c).await,
            Cmd::EditSnippet => self.cmd_edit_snippet(c).await,
            Cmd::Snippets => self.cmd_snippets(c).await,
            Cmd::SuspendCancel => self.cmd_suspend_cancel(c).await,
            Cmd::Suspend => self.cmd_suspend(c).await,
            Cmd::Unsuspend => self.cmd_unsuspend(c).await,
            Cmd::PrivacyPolicyLink => {
                let url = self.self_url("privacy-policy").await;
                c.system_or_say(&format!("<{url}>")).await
            }
            Cmd::Version => {
                c.system_or_say(&format!("Modmail v{} (bot-discord, compatible modmailbot)", env!("CARGO_PKG_VERSION")))
                    .await
            }
            Cmd::NewThread => self.cmd_new_thread(c).await,
            Cmd::Id => self.post_system_message(c.ctx, c.thread(), &c.thread().user_id).await.map(drop),
            Cmd::DmChannelId => {
                let user = parse_id::<UserMarker>(&c.thread().user_id)
                    .ok_or_else(|| anyhow::anyhow!("utilisateur invalide"))?;
                let channel = self.dm_channel(c.ctx, user).await?;
                self.post_system_message(c.ctx, c.thread(), &channel.to_string()).await.map(drop)
            }
            Cmd::MessageInfo => self.cmd_message_info(c).await,
            Cmd::Alert => self.cmd_alert(c).await,
            Cmd::ThreadRole
            | Cmd::ThreadRoleReset
            | Cmd::ThreadRoleSet
            | Cmd::DefaultRole
            | Cmd::DefaultRoleReset
            | Cmd::DefaultRoleSet => self.cmd_role(c, cmd).await,
            Cmd::UserNotes => self.cmd_notes(c, c.args.str("userId").unwrap_or_default()).await,
            Cmd::ThreadNotes => self.cmd_notes(c, &c.thread().user_id).await,
            Cmd::AddUserNote => self.cmd_add_note(c, c.args.str("userId").unwrap_or_default()).await,
            Cmd::AddThreadNote => self.cmd_add_note(c, &c.thread().user_id).await,
            Cmd::DeleteNote => self.cmd_delete_note(c).await,
        }
    }

    async fn cmd_reply(&self, c: &Call<'_>, anonymous: bool) -> Result<()> {
        let text = c.args.str("text").unwrap_or_default();
        if text.is_empty() && c.msg.attachments.is_empty() {
            return c.error("Text or attachment required").await;
        }
        let reference = c.msg.reference.as_ref().and_then(|r| r.message_id);
        if self
            .reply_to_user(
                c.ctx,
                c.thread(),
                &c.moderator(),
                StaffReply { text, attachments: &c.msg.attachments, anonymous, reference },
            )
            .await?
        {
            c.delete_command_message().await;
        }
        Ok(())
    }

    async fn cmd_edit(&self, c: &Call<'_>) -> Result<()> {
        let number = c.args.number("messageNumber").unwrap_or_default();
        let Some(message) = store::message_by_number(&self.db, &c.thread().id, number).await? else {
            return c.error("Unknown message number").await;
        };
        if message.user_id.as_deref() != Some(&c.msg.author.id.to_string()) {
            return c.error("You can only edit your own replies").await;
        }
        if self.edit_staff_reply(c.ctx, c.thread(), &message, c.args.str("text").unwrap_or_default()).await? {
            c.delete_command_message().await;
        }
        Ok(())
    }

    async fn cmd_delete(&self, c: &Call<'_>) -> Result<()> {
        let number = c.args.number("messageNumber").unwrap_or_default();
        let Some(message) = store::message_by_number(&self.db, &c.thread().id, number).await? else {
            return c.error("Unknown message number").await;
        };
        if message.user_id.as_deref() != Some(&c.msg.author.id.to_string()) {
            return c.error("You can only delete your own replies").await;
        }
        self.delete_staff_reply(c.ctx, c.thread(), &message).await?;
        c.delete_command_message().await;
        Ok(())
    }

    async fn cmd_close(&self, c: &Call<'_>) -> Result<()> {
        let is_dm = c.msg.guild_id.is_none();
        let mut silent = false;
        let (suppress_system_messages, closed_by, thread) = if is_dm {
            // Fermeture par l'utilisateur lui-même (si `allowUserClose`).
            if !self.config.allow_user_close || store::is_blocked(&self.db, &c.msg.author.id.to_string()).await? {
                return Ok(());
            }
            let Some(found) = store::find_open_by_user_id(&self.db, &c.msg.author.id.to_string()).await? else {
                return Ok(());
            };
            // Même file que les messages privés : pas de course entre le relais et la fermeture.
            let _queue = self.dm_queue.lock().await;
            self.post_system_message(c.ctx, &found, "Thread closed by user, closing...").await?;
            (true, "the user".to_owned(), found)
        } else {
            if c.msg.guild_id != Some(self.config.inbox_server_id) || !self.is_staff(c.ctx, c.msg) {
                return Ok(());
            }
            let Some(found) =
                store::find_by_channel_and_status(&self.db, &c.msg.channel_id.to_string(), thread_status::OPEN).await?
            else {
                return Ok(());
            };
            let opts = c.args.list("opts");

            if c.args.switch("cancel") || opts.iter().any(|o| o == "cancel" || o == "c") {
                if found.scheduled_close_at.is_some() {
                    store::cancel_scheduled_close(&self.db, &found.id).await?;
                    self.post_system_message(c.ctx, &found, "Cancelled scheduled closing").await?;
                }
                return Ok(());
            }

            silent = c.args.switch("silent") || opts.iter().any(|o| o == "silent" || o == "s");

            if let Some(delay_arg) = opts.iter().find(|o| super::util::DELAY_STRING.is_match(o)) {
                let delay = super::util::convert_delay_string_to_ms(delay_arg).filter(|d| *d != 0);
                let Some(delay) = delay else {
                    return self
                        .post_system_message(c.ctx, &found, "Invalid delay specified. Format: \"1h30m\"")
                        .await
                        .map(drop);
                };
                store::schedule_close(
                    &self.db,
                    &found.id,
                    &db_time_in(delay),
                    &c.msg.author.id.to_string(),
                    &c.author_name(),
                    silent,
                )
                .await?;
                let response = if silent {
                    format!(
                        "Thread is now scheduled to be closed silently in {}. Use `{}close cancel` to cancel.",
                        humanize_delay(delay),
                        self.config.prefix
                    )
                } else {
                    format!(
                        "Thread is now scheduled to be closed in {}. Use `{}close cancel` to cancel.",
                        humanize_delay(delay),
                        self.config.prefix
                    )
                };
                return self.post_system_message(c.ctx, &found, &response).await.map(drop);
            }

            (false, c.author_name(), found)
        };

        self.close_now(c.ctx, &thread, &closed_by, c.msg.author.id, suppress_system_messages, silent).await
    }

    /// Fermeture immédiate : message de fermeture, fermeture, notification dans les logs.
    pub async fn close_now(
        &self,
        ctx: &Ctx,
        thread: &Thread,
        closed_by: &str,
        closed_by_id: Id<UserMarker>,
        suppress_system_messages: bool,
        silent: bool,
    ) -> Result<()> {
        if let (Some(message), false) = (&self.config.close_message, silent) {
            let _ = self.send_system_message_to_user(ctx, thread, message, true).await;
        }
        self.close_thread(ctx, thread, suppress_system_messages, silent).await?;
        self.send_close_notification(
            ctx,
            thread,
            &format!(
                "Modmail thread #{} with {} ({}) was closed by {closed_by} ({closed_by_id})",
                thread.number(),
                thread.user_name,
                thread.user_id,
            ),
        )
        .await
    }

    fn log_query(args: &Args) -> String {
        let mut params = Vec::new();
        if args.switch("verbose") {
            params.push("verbose=1");
        }
        if args.switch("simple") {
            params.push("simple=1");
        }
        params.join("&")
    }

    fn with_query(url: &str, query: &str) -> String {
        if query.is_empty() {
            url.to_owned()
        } else if url.contains('?') {
            format!("{url}&{query}")
        } else {
            format!("{url}?{query}")
        }
    }

    async fn cmd_logs(&self, c: &Call<'_>) -> Result<()> {
        const PER_PAGE: usize = 10;
        let Some(user_id) = c.target_user() else {
            return Ok(());
        };
        let threads = store::closed_threads_by_user(&self.db, &user_id).await?;
        let total = threads.len();
        let max_page = total.div_ceil(PER_PAGE).max(1);
        let page = (c.args.number("page").unwrap_or(1).max(1) as usize).min(max_page);
        let paginated = total > PER_PAGE;
        let start = (page - 1) * PER_PAGE;
        let query = Self::log_query(&c.args);

        let mut lines = Vec::new();
        for thread in threads.iter().skip(start).take(PER_PAGE) {
            let link = match self.log_url(thread).await? {
                Some(url) => format!("<{}>", Self::with_query(&url, &query)),
                None => format!("View log with `{}log {}`", self.config.prefix, thread.number()),
            };
            let date = parse_db_time(&thread.created_at).map(|d| format_log_date(&d)).unwrap_or_default();
            lines.push(format!("`#{}` `{date}`: {link}", thread.number()));
        }

        let mut message = if paginated {
            format!(
                "**Log files for <@{user_id}>** (page **{page}/{max_page}**, showing logs **{}-{}/{total}**):",
                start + 1,
                page * PER_PAGE
            )
        } else {
            format!("**Log files for <@{user_id}>:**")
        };
        message.push_str(&format!("\n{}", lines.join("\n")));
        if paginated {
            message.push_str("\nTo view more, add a page number to the end of the command");
        }
        if lines.is_empty() {
            message = format!("**There are no log files for <@{user_id}>**");
        }

        // Par messages de 15 lignes.
        let all: Vec<&str> = message.split('\n').collect();
        for chunk in chunk_lines(&all, 15) {
            c.say(&chunk).await?;
        }
        Ok(())
    }

    async fn cmd_log(&self, c: &Call<'_>) -> Result<()> {
        let thread = match c.args.str("threadId") {
            Some(id) => match store::find_by_id(&self.db, id).await? {
                Some(thread) => Some(thread),
                None => match id.parse::<i64>() {
                    Ok(number) => store::find_by_thread_number(&self.db, number).await?,
                    Err(_) => None,
                },
            },
            None => c.thread.clone(),
        };
        let Some(thread) = thread else {
            return Ok(());
        };

        if let Some(url) = self.log_url(&thread).await? {
            let url = Self::with_query(&url, &Self::log_query(&c.args));
            return c
                .say(&format!("Open the following link to view the log for thread #{}:\n<{url}>", thread.number()))
                .await;
        }
        if let Some(file) = self.log_file(&thread).await? {
            let uploads =
                [twilight_model::http::attachment::Attachment::from_bytes(file.name, file.content.into_bytes(), 0)];
            c.ctx
                .http
                .create_message(c.channel())
                .content(&format!("Download the following file to view the log for thread #{}:", thread.number()))
                .attachments(&uploads)
                .await?;
            return Ok(());
        }
        if thread.status == thread_status::OPEN {
            let channel = thread.channel_id.unwrap_or_default();
            return c
                .say(&format!("This thread's logs are not currently available, but it's open at <#{channel}>"))
                .await;
        }
        c.say("This thread's logs are not currently available").await
    }

    async fn cmd_block(&self, c: &Call<'_>) -> Result<()> {
        let Some(user_id) = c.target_user() else {
            return Ok(());
        };
        if store::is_blocked(&self.db, &user_id).await? {
            return c.say("User is already blocked").await;
        }
        let block_time = c.args.delay("blockTime").filter(|ms| *ms > 0);
        let expires_at = block_time.map(db_time_in);
        let user_name = match parse_id::<UserMarker>(&user_id) {
            Some(id) => c.ctx.cache.user(id).map(|u| u.name.clone()).unwrap_or_default(),
            None => String::new(),
        };
        store::block(&self.db, &user_id, &user_name, &c.msg.author.id.to_string(), expires_at.as_deref()).await?;

        if let (Some(ms), Some(expires_at)) = (block_time, &expires_at) {
            let humanized = humanize_duration(ms, 2, None);
            c.say(&format!("Blocked <@{user_id}> (id `{user_id}`) from modmail for {humanized}")).await?;
            if let Some(template) = self.config.timed_block_message.as_ref().or(self.config.block_message.as_ref()) {
                let timestamp = parse_db_time(expires_at).map(|d| d.timestamp()).unwrap_or_default();
                let text = template.replace("{duration}", &humanized).replace("{timestamp}", &timestamp.to_string());
                self.dm_user_quietly(c.ctx, &user_id, &text).await;
            }
        } else {
            c.say(&format!("Blocked <@{user_id}> (id `{user_id}`) from modmail indefinitely")).await?;
            if let Some(message) = &self.config.block_message {
                self.dm_user_quietly(c.ctx, &user_id, message).await;
            }
        }
        Ok(())
    }

    async fn cmd_unblock(&self, c: &Call<'_>) -> Result<()> {
        let Some(user_id) = c.target_user() else {
            return Ok(());
        };
        if !store::is_blocked(&self.db, &user_id).await? {
            return c.say("User is not blocked").await;
        }
        if let Some(delay) = c.args.delay("unblockDelay").filter(|ms| *ms > 0) {
            let unblock_at = db_time_in(delay);
            let humanized = humanize_duration(delay, 2, None);
            store::update_block_expiry(&self.db, &user_id, &unblock_at).await?;
            c.say(&format!("Scheduled <@{user_id}> (id `{user_id}`) to be unblocked in {humanized}")).await?;
            if let Some(template) = self.config.timed_unblock_message.as_ref().or(self.config.unblock_message.as_ref())
            {
                let timestamp = parse_db_time(&unblock_at).map(|d| d.timestamp()).unwrap_or_default();
                let text = template.replace("{delay}", &humanized).replace("{timestamp}", &timestamp.to_string());
                self.dm_user_quietly(c.ctx, &user_id, &text).await;
            }
        } else {
            store::unblock(&self.db, &user_id).await?;
            c.say(&format!("Unblocked <@{user_id}> (id {user_id}) from modmail")).await?;
            if let Some(message) = &self.config.unblock_message {
                self.dm_user_quietly(c.ctx, &user_id, message).await;
            }
        }
        Ok(())
    }

    async fn dm_user_quietly(&self, ctx: &Ctx, user_id: &str, content: &str) {
        let Some(user) = parse_id::<UserMarker>(user_id) else {
            return;
        };
        if let Ok(channel) = self.dm_channel(ctx, user).await {
            let _ = ctx.http.create_message(channel).content(content).await;
        }
    }

    async fn cmd_is_blocked(&self, c: &Call<'_>) -> Result<()> {
        let Some(user_id) = c.target_user() else {
            return Ok(());
        };
        let mentions = parse_id(&user_id).map_or(Mentions::None, |id| Mentions::Explicit {
            users: vec![id],
            roles: Vec::new(),
            everyone: false,
        });
        let text = match store::block_status(&self.db, &user_id).await? {
            Some(block) => match block.expires_at {
                Some(expires) => format!("<@!{user_id}> (id `{user_id}`) is blocked until {expires} (UTC)"),
                None => format!("<@!{user_id}> (id `{user_id}`) is blocked indefinitely"),
            },
            None => format!("<@!{user_id}> (id `{user_id}`) is NOT blocked"),
        };
        c.say_with(&text, mentions).await
    }

    async fn cmd_blocklist(&self, c: &Call<'_>) -> Result<()> {
        let blocked = store::blocked_users(&self.db).await?;
        if blocked.is_empty() {
            return c.say("No users are currently blocked.").await;
        }
        let mut reply = "List of blocked users:\n".to_owned();
        for user in blocked {
            let until = user.expires_at.map_or_else(|| " permanently".to_owned(), |e| format!(" until {e} (UTC)"));
            reply.push_str(&format!(
                "**<@!{}> (id `{}`)** - Blocked by <@{}>{until}\n",
                user.user_id,
                user.user_id,
                user.blocked_by.unwrap_or_default()
            ));
        }
        for chunk in chunk_message_lines(&reply) {
            c.say(&chunk).await?;
        }
        Ok(())
    }

    async fn cmd_move(&self, c: &Call<'_>) -> Result<()> {
        let thread = c.thread();
        let search = super::util::slugify(c.args.str("category").unwrap_or_default());
        let current_parent = c.ctx.cache.channel(c.msg.channel_id).and_then(|ch| ch.parent_id);
        let categories: Vec<(Id<ChannelMarker>, String, Vec<PermissionOverwrite>)> = c
            .ctx
            .cache
            .guild_channels(self.config.inbox_server_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| c.ctx.cache.channel(*id))
                    .filter(|ch| ch.kind == ChannelType::GuildCategory && Some(ch.id) != current_parent)
                    .map(|ch| {
                        (
                            ch.id,
                            ch.name.clone().unwrap_or_default(),
                            ch.permission_overwrites.clone().unwrap_or_default(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        if categories.is_empty() {
            return Ok(());
        }

        // Classement : plus long préfixe de la recherche contenu dans le nom,
        // avec un léger bonus pour les catégories qui commencent par la recherche.
        let rank = |name: &str| -> f64 {
            let normalized = super::util::slugify(name);
            let mut i = 0;
            while i < search.len() && normalized.contains(&search[..=i]) {
                i += 1;
            }
            let mut score = i as f64;
            if i > 0 && normalized.starts_with(&search[..i]) {
                score += 0.5;
            }
            score
        };
        let best = categories.iter().map(|cat| (cat, rank(&cat.1))).max_by(|a, b| a.1.total_cmp(&b.1));
        let Some(((category_id, name, overwrites), score)) = best else {
            return Ok(());
        };
        if score == 0.0 {
            return self.post_system_message(c.ctx, thread, "No matching category").await.map(drop);
        }

        let channel_id = c.msg.channel_id;
        if let Err(error) = c.ctx.http.update_channel(channel_id).parent_id(Some(*category_id)).await {
            return self.post_system_message(c.ctx, thread, &format!("Failed to move thread: {error}")).await.map(drop);
        }
        if self.config.sync_permissions_on_move
            && let Err(error) = c.ctx.http.update_channel(channel_id).permission_overwrites(overwrites).await
        {
            return self
                .post_system_message(
                    c.ctx,
                    thread,
                    &format!("Thread moved to {}, but failed to sync permissions: {error}", name.to_uppercase()),
                )
                .await
                .map(drop);
        }
        self.post_system_message(c.ctx, thread, &format!("Thread moved to {}", name.to_uppercase())).await.map(drop)
    }

    async fn cmd_snippet(&self, c: &Call<'_>) -> Result<()> {
        let trigger = c.args.str("trigger").unwrap_or_default();
        let text = c.args.str("text");
        let prefix = &self.config.prefix;
        match (store::snippet(&self.db, trigger).await?, text) {
            (Some(_), Some(_)) => {
                c.system_or_say(&format!(
                    "Snippet \"{trigger}\" already exists! You can edit or delete it with {prefix}edit_snippet and {prefix}delete_snippet respectively."
                ))
                .await
            }
            (Some(snippet), None) => {
                c.system_or_say(&format!("`{}{trigger}` replies with: ```\n{}```", self.config.snippet_prefix, disable_code_blocks(&snippet.body)))
                    .await
            }
            (None, Some(text)) => {
                store::add_snippet(&self.db, trigger, text, &c.msg.author.id.to_string()).await?;
                c.system_or_say(&format!("Snippet \"{trigger}\" created!")).await
            }
            (None, None) => {
                c.system_or_say(&format!("Snippet \"{trigger}\" doesn't exist! You can create it with `{prefix}snippet {trigger} text`")).await
            }
        }
    }

    async fn cmd_delete_snippet(&self, c: &Call<'_>) -> Result<()> {
        let trigger = c.args.str("trigger").unwrap_or_default();
        if store::snippet(&self.db, trigger).await?.is_none() {
            return c.system_or_say(&format!("Snippet \"{trigger}\" doesn't exist!")).await;
        }
        store::delete_snippet(&self.db, trigger).await?;
        c.system_or_say(&format!("Snippet \"{trigger}\" deleted!")).await
    }

    async fn cmd_edit_snippet(&self, c: &Call<'_>) -> Result<()> {
        let trigger = c.args.str("trigger").unwrap_or_default();
        if store::snippet(&self.db, trigger).await?.is_none() {
            return c.system_or_say(&format!("Snippet \"{trigger}\" doesn't exist!")).await;
        }
        store::delete_snippet(&self.db, trigger).await?;
        store::add_snippet(&self.db, trigger, c.args.str("text").unwrap_or_default(), &c.msg.author.id.to_string())
            .await?;
        c.system_or_say(&format!("Snippet \"{trigger}\" edited!")).await
    }

    async fn cmd_snippets(&self, c: &Call<'_>) -> Result<()> {
        let mut triggers: Vec<String> = store::all_snippets(&self.db).await?.into_iter().map(|s| s.trigger).collect();
        triggers.sort();
        c.system_or_say(&format!(
            "Available snippets (prefix {}):\n{}",
            self.config.snippet_prefix,
            triggers.join(", ")
        ))
        .await
    }

    async fn cmd_suspend_cancel(&self, c: &Call<'_>) -> Result<()> {
        let thread = c.thread();
        if thread.scheduled_suspend_at.is_some() {
            store::cancel_scheduled_suspend(&self.db, &thread.id).await?;
            self.post_system_message(c.ctx, thread, "Cancelled scheduled suspension").await.map(drop)
        } else {
            self.post_system_message(c.ctx, thread, "Thread is not scheduled to be suspended").await.map(drop)
        }
    }

    async fn cmd_suspend(&self, c: &Call<'_>) -> Result<()> {
        let thread = c.thread();
        if thread.status == thread_status::SUSPENDED {
            return self.post_system_message(c.ctx, thread, "Thread is already suspended.").await.map(drop);
        }
        if let Some(delay) = c.args.delay("delay").filter(|d| *d > 0) {
            store::schedule_suspend(
                &self.db,
                &thread.id,
                &db_time_in(delay),
                &c.msg.author.id.to_string(),
                &c.author_name(),
            )
            .await?;
            return self
                .post_system_message(
                    c.ctx,
                    thread,
                    &format!(
                        "Thread will be suspended in {}. Use `{}suspend cancel` to cancel.",
                        humanize_delay(delay),
                        self.config.prefix
                    ),
                )
                .await
                .map(drop);
        }
        store::suspend(&self.db, &thread.id).await?;
        self.post_system_message(
            c.ctx,
            thread,
            &format!(
                "**Thread suspended!** This thread will act as closed until unsuspended with `{}unsuspend`",
                self.config.prefix
            ),
        )
        .await
        .map(drop)
    }

    async fn cmd_unsuspend(&self, c: &Call<'_>) -> Result<()> {
        if let Some(thread) = &c.thread {
            return self.post_system_message(c.ctx, thread, "Thread is not suspended").await.map(drop);
        }
        let Some(thread) =
            store::find_by_channel_and_status(&self.db, &c.msg.channel_id.to_string(), thread_status::SUSPENDED)
                .await?
        else {
            return c.say("Not in a thread").await;
        };
        if let Some(other) = store::find_open_by_user_id(&self.db, &thread.user_id).await? {
            let channel = other.channel_id.unwrap_or_default();
            return self
                .post_system_message(
                    c.ctx,
                    &thread,
                    &format!("Cannot unsuspend; there is another open thread with this user: <#{channel}>"),
                )
                .await
                .map(drop);
        }
        store::set_status(&self.db, &thread.id, thread_status::OPEN).await?;
        self.post_system_message(c.ctx, &thread, "**Thread unsuspended!**").await.map(drop)
    }

    async fn cmd_new_thread(&self, c: &Call<'_>) -> Result<()> {
        let Some(user_id) = c.args.str("userId").and_then(parse_id::<UserMarker>) else {
            return c.system_or_say("User not found!").await;
        };
        let user = match c.ctx.http.user(user_id).await {
            Ok(response) => response.model().await?,
            Err(_) => return c.system_or_say("User not found!").await,
        };
        if user.bot {
            return c.system_or_say("Can't create a thread for a bot").await;
        }
        if let Some(existing) = store::find_open_by_user_id(&self.db, &user.id.to_string()).await? {
            let channel = existing.channel_id.unwrap_or_default();
            return c
                .system_or_say(&format!(
                    "Cannot create a new thread; there is another open thread with this user: <#{channel}>"
                ))
                .await;
        }

        let Some(created) = self
            .create_new_thread_for_user(c.ctx, &user, NewThreadOptions { quiet: true, ignore_requirements: true })
            .await?
        else {
            return Ok(());
        };
        let opener = c.msg.member.as_ref().and_then(|m| m.nick.clone()).unwrap_or_else(|| c.author_name());
        self.post_system_message(c.ctx, &created, &format!("Thread was opened by {opener}")).await?;
        c.say(&format!("Thread opened: <#{}>", created.channel_id.unwrap_or_default())).await
    }

    async fn cmd_message_info(&self, c: &Call<'_>) -> Result<()> {
        let thread = c.thread();
        let number = c.args.number("messageNumber").unwrap_or_default();
        let Some(message) = store::message_by_number(&self.db, &thread.id, number).await? else {
            return self
                .post_system_message(c.ctx, thread, "No message in this thread with that number")
                .await
                .map(drop);
        };
        let channel_id = message.dm_channel_id.clone().unwrap_or_default();
        // Cas rare (createThreadOnMention) : le message vient d'un serveur principal.
        let guild = parse_id::<ChannelMarker>(&channel_id)
            .and_then(|id| c.ctx.cache.channel(id))
            .and_then(|ch| ch.guild_id)
            .filter(|g| self.config.is_main_server(*g));
        let link = match guild {
            Some(guild) => format!(
                "https://discord.com/channels/{guild}/{channel_id}/{}",
                message.dm_message_id.clone().unwrap_or_default()
            ),
            None => format!(
                "https://discord.com/channels/@me/{channel_id}/{}",
                message.dm_message_id.clone().unwrap_or_default()
            ),
        };
        let parts = [
            format!("Details for message `{}`:", message.message_number.unwrap_or_default()),
            format!("Channel ID: `{channel_id}`"),
            format!("Message ID: `{}`", message.dm_message_id.unwrap_or_default()),
            format!("Link: <{link}>"),
        ];
        self.post_system_message(c.ctx, thread, &parts.join("\n")).await.map(drop)
    }

    async fn cmd_alert(&self, c: &Call<'_>) -> Result<()> {
        let thread = c.thread();
        if c.args.str("opt").is_some_and(|opt| opt.starts_with('c')) {
            store::remove_alert(&self.db, &thread.id, &c.msg.author.id.to_string()).await?;
            return self.post_system_message(c.ctx, thread, "Cancelled new message alert").await.map(drop);
        }
        store::add_alert(&self.db, &thread.id, &c.msg.author.id.to_string()).await?;
        let name = c.msg.member.as_ref().and_then(|m| m.nick.clone()).unwrap_or_else(|| c.author_name());
        self.post_system_message(c.ctx, thread, &format!("Pinging {name} when this thread gets a new reply"))
            .await
            .map(drop)
    }

    fn resolve_role(&self, ctx: &Ctx, input: &str) -> Option<(Id<RoleMarker>, String)> {
        let roles = ctx.cache.guild_roles(self.config.inbox_server_id)?;
        let found = if is_snowflake(input) {
            parse_id::<RoleMarker>(input).filter(|id| roles.contains(id))
        } else {
            roles
                .iter()
                .copied()
                .find(|id| ctx.cache.role(*id).is_some_and(|r| r.resource().name.eq_ignore_ascii_case(input)))
        }?;
        ctx.cache.role(found).map(|r| (found, r.resource().name.clone()))
    }

    async fn cmd_role(&self, c: &Call<'_>, cmd: Cmd) -> Result<()> {
        let moderator = c.moderator();
        let moderator_id = moderator.id().to_string();
        let prefix = &self.config.prefix;
        match cmd {
            Cmd::ThreadRole => {
                let thread = c.thread();
                let text = match self.moderator_display_role(c.ctx, &moderator, Some(&thread.id)).await? {
                    Some(role) => format!("Your display role in this thread is currently **{role}**"),
                    None => "Your replies in this thread do not currently display a role".to_owned(),
                };
                self.post_system_message(c.ctx, thread, &text).await.map(drop)
            }
            Cmd::ThreadRoleReset => {
                let thread = c.thread();
                store::reset_role_override(&self.db, &moderator_id, Some(&thread.id)).await?;
                let text = match self.moderator_display_role(c.ctx, &moderator, Some(&thread.id)).await? {
                    Some(role) => format!(
                        "Your display role for this thread has been reset. Your replies will now display the default role **{role}**."
                    ),
                    None => {
                        "Your display role for this thread has been reset. Your replies will no longer display a role."
                            .to_owned()
                    }
                };
                self.post_system_message(c.ctx, thread, &text).await.map(drop)
            }
            Cmd::ThreadRoleSet => {
                let thread = c.thread();
                let role = self
                    .resolve_role(c.ctx, c.args.str("role").unwrap_or_default())
                    .filter(|(id, _)| moderator.roles.contains(id));
                let Some((role_id, name)) = role else {
                    return self
                        .post_system_message(
                            c.ctx,
                            thread,
                            "No matching role found. Make sure you have the role before trying to set it as your display role in this thread.",
                        )
                        .await
                        .map(drop);
                };
                store::set_role_override(&self.db, &moderator_id, Some(&thread.id), &role_id.to_string()).await?;
                self.post_system_message(
                    c.ctx,
                    thread,
                    &format!("Your display role for this thread has been set to **{name}**. You can reset it with `{prefix}role reset`."),
                )
                .await
                .map(drop)
            }
            Cmd::DefaultRole => {
                let text = match self.moderator_display_role(c.ctx, &moderator, None).await? {
                    Some(role) => format!("Your default display role is currently **{role}**"),
                    None => "Your replies do not currently display a role by default".to_owned(),
                };
                c.say(&text).await
            }
            Cmd::DefaultRoleReset => {
                store::reset_role_override(&self.db, &moderator_id, None).await?;
                let text = match self.moderator_display_role(c.ctx, &moderator, None).await? {
                    Some(role) => format!("Your default display role has been reset. Your replies will now display the role **{role}** by default."),
                    None => "Your default display role has been reset. Your replies will no longer display a role by default.".to_owned(),
                };
                c.say(&text).await
            }
            Cmd::DefaultRoleSet => {
                let role = self
                    .resolve_role(c.ctx, c.args.str("role").unwrap_or_default())
                    .filter(|(id, _)| moderator.roles.contains(id));
                let Some((role_id, name)) = role else {
                    return c.say("No matching role found. Make sure you have the role before trying to set it as your default display role.").await;
                };
                store::set_role_override(&self.db, &moderator_id, None, &role_id.to_string()).await?;
                c.say(&format!(
                    "Your default display role has been set to **{name}**. You can reset it with `{prefix}role reset`."
                ))
                .await
            }
            _ => Ok(()),
        }
    }

    async fn cmd_notes(&self, c: &Call<'_>, user_id: &str) -> Result<()> {
        let notes = store::notes_by_user(&self.db, user_id).await?;
        if notes.is_empty() {
            return c.say(&format!("There are no notes for <@!{user_id}>")).await;
        }
        for note in notes {
            let timestamp =
                note.created_at.as_deref().and_then(parse_db_time).map(|d| d.timestamp()).unwrap_or_default();
            let content = [
                format!("Set by <@!{}> at <t:{timestamp}:f>:", note.author_id.unwrap_or_default()),
                format!("```{}```", escape_markdown(note.body.as_deref().unwrap_or_default())),
                format!("*Delete with `{}delete_note {}`*\n", self.config.prefix, note.id),
            ]
            .join("\n");
            // Ne pas pinguer chaque auteur de note.
            for chunk in chunk_message_lines(&content) {
                c.say(&chunk).await?;
            }
        }
        Ok(())
    }

    async fn cmd_add_note(&self, c: &Call<'_>, user_id: &str) -> Result<()> {
        store::create_note(&self.db, user_id, &c.msg.author.id.to_string(), c.args.str("body").unwrap_or_default())
            .await?;
        c.say(&format!("Note added for <@!{user_id}>")).await
    }

    async fn cmd_delete_note(&self, c: &Call<'_>) -> Result<()> {
        let id = c.args.number("noteId").unwrap_or_default();
        let Some(note) = store::note(&self.db, id).await? else {
            return c.error("Note not found!").await;
        };
        store::delete_note(&self.db, id).await?;
        c.say(&format!(
            "Deleted note on <@!{}>:\n```{}```",
            note.user_id.unwrap_or_default(),
            escape_markdown(note.body.as_deref().unwrap_or_default())
        ))
        .await
    }
}

/// « MMM Do [at] HH:mm [UTC] » de moment.js, ex. « Jan 2nd at 03:04 UTC ».
fn format_log_date(date: &chrono::DateTime<chrono::Utc>) -> String {
    use chrono::Datelike;
    let day = date.day();
    let suffix = match (day % 10, day % 100) {
        (1, 11) | (2, 12) | (3, 13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{} {day}{suffix} at {} UTC", date.format("%b"), date.format("%H:%M"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn formats_dates_like_moment() {
        let date = chrono::Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 0).unwrap();
        assert_eq!(format_log_date(&date), "Jan 2nd at 03:04 UTC");
        let date = chrono::Utc.with_ymd_and_hms(2026, 3, 11, 13, 0, 0).unwrap();
        assert_eq!(format_log_date(&date), "Mar 11th at 13:00 UTC");
    }
}
