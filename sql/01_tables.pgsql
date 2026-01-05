CREATE TABLE IF NOT EXISTS users (
    user_id BIGINT PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role_id INT DEFAULT 0,
    followers_count BIGINT NOT NULL DEFAULT 0,
    following_count BIGINT NOT NULL DEFAULT 0,
    email_verified BOOLEAN DEFAULT FALSE,
    pending_email TEXT,
    pending_email_until TIMESTAMPTZ
);


CREATE TABLE IF NOT EXISTS user_notifications (
    id BIGINT PRIMARY KEY,
    user_id BIGINT NOT NULL,
    type TEXT NOT NULL,
    message TEXT,
    from_id BIGINT NOT NULL,
    linked_type TEXT,
    linked_id BIGINT,
    second_linked_id BIGINT,
    unread BOOLEAN DEFAULT TRUE,
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE,
    FOREIGN KEY (from_id) REFERENCES users(user_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS auth_keys (
    session_id BIGINT PRIMARY KEY,
    user_id BIGINT NOT NULL,
    token_secret TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE,
    UNIQUE (token_secret, user_id, session_id)
);

CREATE TABLE IF NOT EXISTS files (
    context_id BIGINT PRIMARY KEY,
    user_id BIGINT NOT NULL,
    objects TEXT[] NOT NULL,
    reference_count INT NOT NULL DEFAULT 0,
    allowed_count INT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    type TEXT NOT NULL DEFAULT 'context',  -- "avatar" | "banner" | "post_video" | "post_image" | any
    FOREIGN KEY (user_id) REFERENCES users (user_id)
);

CREATE TABLE IF NOT EXISTS posts (
    post_id BIGINT PRIMARY KEY,
    user_id BIGINT NOT NULL,
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMPTZ DEFAULT NULL,
    likes_count BIGINT DEFAULT 0,
    dislikes_count BIGINT DEFAULT 0,
    comments_count BIGINT DEFAULT 0,
    popularity_score BIGINT GENERATED ALWAYS AS (likes_count - dislikes_count + (comments_count * 0.25)) STORED,
    flags TEXT[],
    file_context_id BIGINT,
    status VARCHAR(20) DEFAULT 'active',
    is_deleted BOOLEAN DEFAULT FALSE,
    FOREIGN KEY (user_id) REFERENCES users (user_id) ON DELETE CASCADE,
    FOREIGN KEY (file_context_id) REFERENCES files(context_id)
);

CREATE TABLE IF NOT EXISTS user_post_views (
    user_id BIGINT NOT NULL,
    post_id BIGINT NOT NULL,
    timestamp TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (user_id, post_id),
    FOREIGN KEY (user_id) REFERENCES users (user_id) ON DELETE CASCADE,
    FOREIGN KEY (post_id) REFERENCES posts (post_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS comments (
    comment_id BIGINT PRIMARY KEY,
    parent_comment_id BIGINT,
    post_id BIGINT NOT NULL,
    user_id BIGINT,
    content TEXT,
    likes_count BIGINT DEFAULT 0,
    dislikes_count BIGINT DEFAULT 0,
    replies_count BIGINT DEFAULT 0,
    popularity_score BIGINT GENERATED ALWAYS AS (likes_count - dislikes_count + (replies_count * 0.25)) STORED,
    type TEXT DEFAULT 'comment',
    FOREIGN KEY (parent_comment_id) REFERENCES comments (comment_id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users (user_id) ON DELETE CASCADE,
    FOREIGN KEY (post_id) REFERENCES posts (post_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS reactions (
    post_id BIGINT NOT NULL,
    comment_id BIGINT,
    user_id BIGINT NOT NULL,
    is_like BOOLEAN NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (post_id, comment_id, user_id),
    FOREIGN KEY (user_id) REFERENCES users (user_id) ON DELETE CASCADE,
    FOREIGN KEY (post_id) REFERENCES posts (post_id) ON DELETE CASCADE,
    FOREIGN KEY (comment_id) REFERENCES comments (comment_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS favorites (
    user_id BIGINT NOT NULL,
    post_id BIGINT NOT NULL,
    comment_id BIGINT,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (post_id, comment_id, user_id),
    FOREIGN KEY (user_id) REFERENCES users (user_id) ON DELETE CASCADE,
    FOREIGN KEY (post_id) REFERENCES posts (post_id) ON DELETE CASCADE,
    FOREIGN KEY (comment_id) REFERENCES comments (comment_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS followed (
    user_id BIGINT NOT NULL,
    followed_to TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (user_id, followed_to),
    FOREIGN KEY (user_id) REFERENCES users (user_id) ON DELETE CASCADE,
    FOREIGN KEY (followed_to) REFERENCES users (user_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS user_profiles (
    user_id BIGINT PRIMARY KEY,
    display_name TEXT,
    banner_context_id BIGINT,
    avatar_context_id BIGINT,
    bio TEXT,
    languages TEXT[],
    badges SMALLINT[],
    FOREIGN KEY (user_id) REFERENCES users (user_id) ON DELETE CASCADE,
    FOREIGN KEY (banner_context_id) REFERENCES files(context_id),
    FOREIGN KEY (avatar_context_id) REFERENCES files(context_id)

);

CREATE TABLE IF NOT EXISTS tags (
    tag_id BIGINT PRIMARY KEY,
    name TEXT UNIQUE NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    posts_count BIGINT DEFAULT 0
);

CREATE TABLE IF NOT EXISTS post_tags (
    post_id BIGINT NOT NULL REFERENCES posts(post_id) ON DELETE CASCADE,
    tag_id BIGINT NOT NULL REFERENCES tags(tag_id) ON DELETE CASCADE,
    PRIMARY KEY (post_id, tag_id)
);

CREATE TABLE IF NOT EXISTS reports (
    report_id BIGINT PRIMARY KEY,
    user_id BIGINT NOT NULL,
    target_id BIGINT NOT NULL,
    target_type TEXT NOT NULL
        CHECK (target_type IN ('post', 'comment', 'user', 'message')),
    reason TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'reviewed')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS push_subscriptions (
    id BIGINT PRIMARY KEY,
    type TEXT NOT NULL,
    user_id BIGINT NOT NULL,
    session_id BIGINT NOT NULL,
    expiration_time TIMESTAMPTZ,
    raw JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE (session_id),
    FOREIGN KEY (user_id) REFERENCES users (user_id) ON DELETE CASCADE,
    FOREIGN KEY (session_id) REFERENCES auth_keys (session_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS mod_audit (
    id BIGINT PRIMARY KEY,
    user_id BIGINT NOT NULL,
    towards_to TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT now(),
    metadata JSONB,
    old_content JSONB,
    target_type TEXT NOT NULL,
    target_id BIGINT NOT NULL,
    action_type TEXT NOT NULL,
    reason TEXT NOT NULL,
    role_id SMALLINT NOT NULL,
    appellation_status TEXT NOT NULL DEFAULT 'none'
        CHECK (appellation_status IN ('none', 'pending', 'rejected', 'approved'))
);

CREATE TABLE IF NOT EXISTS mod_assigned_resources (
    resource_id BIGINT NOT NULL,
    resource_type TEXT NOT NULL
        CHECK (resource_type IN ('post', 'comment', 'user', 'message', 'appellation')),
    assigned_to TEXT NOT NULL,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (resource_id, resource_type),
    FOREIGN KEY (assigned_to) REFERENCES users (user_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS channels (
    channel_id BIGINT PRIMARY KEY,
    metadata JSONB,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    type TEXT NOT NULL
        CHECK (type IN ('group', 'direct'))
);

CREATE TABLE IF NOT EXISTS messages (
    message_id BIGINT PRIMARY KEY,
    channel_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    content TEXT NOT NULL,
    content_type TEXT NOT NULL
        CHECK (content_type IN ('plain', 'encrypted'))
        DEFAULT 'plain',
    file_context_id BIGINT,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    edited_at TIMESTAMPTZ,
    FOREIGN KEY (channel_id) REFERENCES channels(channel_id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE,
    FOREIGN KEY (file_context_id) REFERENCES files(context_id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS channel_members (
    membership_id BIGINT PRIMARY KEY,
    channel_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    joined_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (channel_id, user_id),
    FOREIGN KEY (channel_id) REFERENCES channels(channel_id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS user_channels (
    user_id BIGINT NOT NULL,
    channel_id BIGINT NOT NULL,
    membership_id BIGINT PRIMARY KEY,
    last_read_message_id BIGINT,
    last_read_at TIMESTAMPTZ,
    UNIQUE (user_id, channel_id),
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE,
    FOREIGN KEY (channel_id) REFERENCES channels(channel_id) ON DELETE CASCADE,
    FOREIGN KEY (last_read_message_id) REFERENCES messages(message_id) ON DELETE SET NULL,
    FOREIGN KEY (membership_id) REFERENCES channel_members(membership_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS friends (
    user_id BIGINT NOT NULL,
    friend_id BIGINT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, friend_id),
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE,
    FOREIGN KEY (friend_id) REFERENCES users(user_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS friend_requests (
    request_id BIGINT PRIMARY KEY,
    from_user_id BIGINT NOT NULL,
    to_user_id BIGINT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (from_user_id) REFERENCES users(user_id) ON DELETE CASCADE,
    FOREIGN KEY (to_user_id) REFERENCES users(user_id) ON DELETE CASCADE
);
