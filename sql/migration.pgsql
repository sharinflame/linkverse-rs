-- =========================================
-- Migration: LinkVerse -> LinkVerse-rs
-- DELETES ALL TRIGGERS AND VIEWS, ADD THEM AFTER MIGRATION
-- =========================================

-- Starting transaction in case something breaks
BEGIN;

-- 0. Delete all triggers and views
DROP TRIGGER IF EXISTS update_posts_modified        ON posts;
DROP TRIGGER IF EXISTS update_posts_deleted_at      ON posts;
DROP TRIGGER IF EXISTS reports_updated_at           ON reports;
DROP TRIGGER IF EXISTS trigger_likes_insert         ON reactions;
DROP TRIGGER IF EXISTS trigger_likes_delete         ON reactions;
DROP TRIGGER IF EXISTS trigger_likes_update         ON reactions;
DROP TRIGGER IF EXISTS trigger_comments_insert      ON comments;
DROP TRIGGER IF EXISTS trigger_comments_delete      ON comments;
DROP TRIGGER IF EXISTS trigger_check_linked_id      ON user_notifications;
DROP TRIGGER IF EXISTS trigger_delete_resources_on_comment_delete ON comments;
DROP TRIGGER IF EXISTS trigger_delete_resources_on_post_delete    ON posts;
DROP TRIGGER IF EXISTS trigger_delete_resources_on_audit_delete   ON mod_audit;
DROP TRIGGER IF EXISTS trigger_replies_insert       ON comments;
DROP TRIGGER IF EXISTS trigger_replies_delete       ON comments;
DROP TRIGGER IF EXISTS trigger_tag_posts_count_increment ON post_tags;
DROP TRIGGER IF EXISTS trigger_tag_posts_count_decrement ON post_tags;
DROP TRIGGER IF EXISTS posts_file_refcount          ON posts;
DROP TRIGGER IF EXISTS user_avatar_file_refcount    ON user_profiles;
DROP TRIGGER IF EXISTS user_banner_file_refcount    ON user_profiles;
DROP TRIGGER IF EXISTS trigger_follow_counts        ON followed;
DROP VIEW IF EXISTS user_channel_view;

-- 1. Renaming posts.tags to posts.flags
ALTER TABLE posts RENAME COLUMN tags TO flags;

-- 2. Renaming webpush_subscriptions
ALTER TABLE webpush_subscriptions RENAME TO push_subscriptions;
ALTER TABLE push_subscriptions ADD COLUMN type TEXT;

-- 3. Drop ::bigint indexes
DROP INDEX users_id_num_idx;
DROP INDEX profiles_id_num_idx;
DROP INDEX notifications_id_num_idx;
DROP INDEX posts_id_num_idx;
DROP INDEX comments_id_num_idx;
DROP INDEX tag_id_num_idx;
DROP INDEX message_id_num_idx;

-- 4. Migrate ids to bigint
DO $$
DECLARE
    tables text[] := ARRAY[
        'users','user_notifications','auth_keys','files','posts','user_post_views',
        'comments','reactions','favorites','followed','user_profiles','tags','post_tags',
        'reports','push_subscriptions','mod_audit','mod_assigned_resources','channels',
        'messages','channel_members','user_channels','friends','friend_requests'
    ];

    cols text[] := ARRAY[
        'users.user_id',
        'user_notifications.id',
        'user_notifications.user_id',
        'user_notifications.from_id',
        'auth_keys.session_id',
        'auth_keys.user_id',
        'files.context_id',
        'files.user_id',
        'posts.post_id',
        'posts.user_id',
        'posts.file_context_id',
        'user_post_views.user_id',
        'user_post_views.post_id',
        'comments.comment_id',
        'comments.parent_comment_id',
        'comments.post_id',
        'comments.user_id',
        'reactions.post_id',
        'reactions.comment_id',
        'reactions.user_id',
        'favorites.user_id',
        'favorites.post_id',
        'favorites.comment_id',
        'followed.user_id',
        'followed.followed_to',
        'user_profiles.user_id',
        'user_profiles.banner_context_id',
        'user_profiles.avatar_context_id',
        'tags.tag_id',
        'post_tags.post_id',
        'post_tags.tag_id',
        'reports.report_id',
        'reports.user_id',
        'reports.target_id',
        'push_subscriptions.id',
        'push_subscriptions.user_id',
        'push_subscriptions.session_id',
        'mod_audit.id',
        'mod_audit.user_id',
        'mod_audit.towards_to',
        'mod_audit.target_id',
        'mod_assigned_resources.resource_id',
        'mod_assigned_resources.assigned_to',
        'channels.channel_id',
        'messages.message_id',
        'messages.channel_id',
        'messages.user_id',
        'messages.file_context_id',
        'channel_members.membership_id',
        'channel_members.channel_id',
        'channel_members.user_id',
        'user_channels.user_id',
        'user_channels.channel_id',
        'user_channels.membership_id',
        'user_channels.last_read_message_id',
        'friends.user_id',
        'friends.friend_id',
        'friend_requests.request_id',
        'friend_requests.from_user_id',
        'friend_requests.to_user_id'
    ];

    fk_rec record;
    tbl text;
    col text;
    invalid_count int;
BEGIN
    RAISE NOTICE 'Starting migration: saving FK defs and dropping them...';

    CREATE TEMP TABLE tmp_fk_defs (tbl text, conname text, condef text) ON COMMIT DROP;

    INSERT INTO tmp_fk_defs (tbl, conname, condef)
    SELECT conrelid::regclass::text AS tbl,
           conname,
           pg_get_constraintdef(oid)
    FROM pg_constraint
    WHERE contype = 'f'
      AND (
           conrelid::regclass::text = ANY(tables)
        OR confrelid::regclass::text = ANY(tables)
      );

    FOR fk_rec IN SELECT * FROM tmp_fk_defs LOOP
        RAISE NOTICE 'Dropping constraint % on table %', fk_rec.conname, fk_rec.tbl;
        EXECUTE format('ALTER TABLE %I DROP CONSTRAINT IF EXISTS %I', fk_rec.tbl, fk_rec.conname);
    END LOOP;

    RAISE NOTICE 'All related foreign keys dropped (saved in tmp_fk_defs).';

    FOR i IN array_lower(cols,1)..array_upper(cols,1) LOOP
        tbl := split_part(cols[i], '.', 1);
        col := split_part(cols[i], '.', 2);

        EXECUTE format(
            'SELECT count(*) FROM %I WHERE %I IS NOT NULL AND %I !~ %L',
            tbl, col, col, '^[0-9]+$'
        ) INTO invalid_count;

        IF invalid_count IS NULL THEN
            invalid_count := 0;
        END IF;

        IF invalid_count > 0 THEN
            RAISE EXCEPTION 'Found % non-numeric values in %.%. Clean data before running migration.', invalid_count, tbl, col;
        END IF;

        RAISE NOTICE 'Altering %.% -> BIGINT', tbl, col;
        EXECUTE format('ALTER TABLE %I ALTER COLUMN %I TYPE BIGINT USING (%I::bigint)', tbl, col, col);
    END LOOP;

    RAISE NOTICE 'All specified columns altered to BIGINT.';

    FOR fk_rec IN SELECT * FROM tmp_fk_defs LOOP
        RAISE NOTICE 'Recreating constraint % on table %', fk_rec.conname, fk_rec.tbl;
        EXECUTE format('ALTER TABLE %I ADD CONSTRAINT %I %s', fk_rec.tbl, fk_rec.conname, fk_rec.condef);
    END LOOP;

    RAISE NOTICE 'Foreign keys restored.';

END;
$$ LANGUAGE plpgsql;

-- 5. Change constraints
ALTER TABLE user_notifications
    DROP CONSTRAINT user_notifications_from_id_fkey,
    ADD CONSTRAINT user_notifications_from_id_fkey
    FOREIGN KEY (from_id) REFERENCES users(user_id);

ALTER TABLE comments
    DROP CONSTRAINT comments_user_id_fkey,
    ADD CONSTRAINT comments_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(user_id);

ALTER TABLE reactions
    DROP CONSTRAINT reactions_user_id_fkey,
    ADD CONSTRAINT reactions_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(user_id);

ALTER TABLE favorites
    DROP CONSTRAINT favorites_user_id_fkey,
    ADD CONSTRAINT favorites_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(user_id);

ALTER TABLE messages
    DROP CONSTRAINT messages_user_id_fkey,
    ADD CONSTRAINT messages_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(user_id);

-- 6. Update user channels (we don't have any data, right?)
DROP TABLE IF EXISTS user_channels;

ALTER TABLE channel_members
ADD COLUMN IF NOT EXISTS last_read_message_id BIGINT,
ADD COLUMN IF NOT EXISTS badge_counter SMALLINT DEFAULT 0;

ALTER TABLE channel_members
ADD CONSTRAINT fk_last_read_message
FOREIGN KEY (last_read_message_id)
REFERENCES messages(message_id);

-- 7. Commit everything
COMMIT;

-- 8. Reindex and analyze tables
DO $$
DECLARE
    tbl text;
    tables_to_fix text[] := ARRAY[
        'users','user_notifications','auth_keys','files','posts','user_post_views',
        'comments','reactions','favorites','followed','user_profiles','tags','post_tags',
        'reports','push_subscriptions','mod_audit','mod_assigned_resources','channels',
        'messages','channel_members','user_channels','friends','friend_requests'
    ];
BEGIN
    FOREACH tbl IN ARRAY tables_to_fix LOOP
        RAISE NOTICE 'Reindexing and analyzing %', tbl;
        EXECUTE format('REINDEX TABLE %I', tbl);
        EXECUTE format('ANALYZE %I', tbl);
    END LOOP;
END;
$$ LANGUAGE plpgsql;

