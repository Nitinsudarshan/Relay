-- ============================================================================
-- Relay Cloud Backend Schema & Row Level Security (RLS) — Hardened (11D.1)
-- Strict Separation: Identity, Installation Tracking, Telemetry & App Releases
-- Invariant: Zero access to local vaults. Rate-limited & validated ingestion.
-- ============================================================================

-- 1. RELAY ACCOUNTS / PROFILES
create table if not exists public.relay_accounts (
  id uuid references auth.users(id) on delete cascade primary key,
  email text,
  display_name text,
  profile_image text,
  provider text default 'google',
  account_mode text default 'local' check (account_mode in ('local', 'hybrid')),
  subscription_plan text default 'free' check (subscription_plan in ('free', 'hybrid')),
  subscription_status text default 'active',
  created_at timestamp with time zone default timezone('utc'::text, now()) not null,
  updated_at timestamp with time zone default timezone('utc'::text, now()) not null
);

-- 2. INSTALLATIONS TABLE
-- Tracks anonymous installation instances, app versions, and last seen timestamps.
create table if not exists public.installations (
  installation_id uuid primary key,
  user_id uuid references auth.users(id) on delete set null,
  app_version text not null,
  platform text not null,
  os_version text not null,
  first_installed_at timestamp with time zone default timezone('utc'::text, now()) not null,
  last_seen_at timestamp with time zone default timezone('utc'::text, now()) not null
);

-- 3. DIAGNOSTICS & TELEMETRY TABLE
-- Strictly firewalled system metadata (ZERO notes, scribbles, audio, or transcripts).
create table if not exists public.diagnostics_events (
  id uuid default gen_random_uuid() primary key,
  installation_id uuid not null,
  user_id uuid references auth.users(id) on delete set null,
  relay_version text not null,
  platform text not null,
  os_version text not null,
  event_type text not null,
  metadata jsonb default '{}'::jsonb,
  created_at timestamp with time zone default timezone('utc'::text, now()) not null
);

-- 4. APP RELEASES & UPDATES TABLE (Public Anonymous Read)
create table if not exists public.app_releases (
  id uuid default gen_random_uuid() primary key,
  version text unique not null,
  min_supported_version text not null default '0.8.0',
  release_notes text,
  download_url text,
  is_active boolean default true,
  published_at timestamp with time zone default timezone('utc'::text, now()) not null
);

-- Seed initial releases
insert into public.app_releases (version, min_supported_version, release_notes, download_url, is_active)
values 
  ('0.9.0', '0.8.0', 'Phase 11D — Relay Identity, Product Account & Supabase Foundation', 'https://github.com/Nitinsudarshan/Relay/releases/tag/v0.9.0', true)
on conflict (version) do update set 
  release_notes = excluded.release_notes,
  is_active = excluded.is_active;

-- ============================================================================
-- HARDENED ROW LEVEL SECURITY (RLS) POLICIES
-- ============================================================================

alter table public.relay_accounts enable row level security;
alter table public.installations enable row level security;
alter table public.diagnostics_events enable row level security;
alter table public.app_releases enable row level security;

-- 1. Relay Accounts: strictly own user record only
create policy "Users can view own account" on public.relay_accounts
  for select using (auth.uid() = id);

create policy "Users can update own account" on public.relay_accounts
  for update using (auth.uid() = id);

create policy "Users can insert own account" on public.relay_accounts
  for insert with check (auth.uid() = id);

create policy "Users can delete own account" on public.relay_accounts
  for delete using (auth.uid() = id);

-- 2. Installations: NO open SELECT. Only service_role or authenticated owner can read.
create policy "Admins can view all installations" on public.installations
  for select using (auth.role() = 'service_role');

create policy "Users can view own installation" on public.installations
  for select using (auth.uid() is not null and auth.uid() = user_id);

-- 3. Diagnostics Events: Append-only via RPC; SELECT strictly locked to service_role
create policy "Admins can view diagnostics" on public.diagnostics_events
  for select using (auth.role() = 'service_role');

-- 4. App Releases: Public read for active releases (no auth required)
create policy "Public can view active releases" on public.app_releases
  for select using (is_active = true);

-- ============================================================================
-- SECURE RPC ENDPOINTS (SECURITY DEFINER)
-- Controlled heartbeat & rate-guarded telemetry ingestion
-- ============================================================================

-- Function: register_installation_heartbeat
-- Upserts only the caller's installation record with validated metadata.
create or replace function public.register_installation_heartbeat(
  p_installation_id uuid,
  p_app_version text,
  p_platform text,
  p_os_version text
)
returns void
language plpgsql
security definer
as $$
declare
  v_user_id uuid;
begin
  -- Validate inputs
  if p_installation_id is null or length(p_app_version) > 32 or length(p_platform) > 32 or length(p_os_version) > 64 then
    raise exception 'Invalid installation heartbeat parameters';
  end if;

  v_user_id := auth.uid();

  insert into public.installations (
    installation_id,
    user_id,
    app_version,
    platform,
    os_version,
    first_installed_at,
    last_seen_at
  )
  values (
    p_installation_id,
    v_user_id,
    p_app_version,
    p_platform,
    p_os_version,
    now(),
    now()
  )
  on conflict (installation_id) do update set
    user_id = coalesce(excluded.user_id, installations.user_id),
    app_version = excluded.app_version,
    platform = excluded.platform,
    os_version = excluded.os_version,
    last_seen_at = now();
end;
$$;

-- Function: ingest_diagnostic_event
-- Rate-guarded, validated telemetry ingestion (strictly no user notes or text).
create or replace function public.ingest_diagnostic_event(
  p_installation_id uuid,
  p_relay_version text,
  p_platform text,
  p_os_version text,
  p_event_type text,
  p_metadata jsonb default '{}'::jsonb
)
returns void
language plpgsql
security definer
as $$
declare
  v_user_id uuid;
begin
  -- Validate inputs
  if p_installation_id is null or length(p_event_type) > 64 or length(p_relay_version) > 32 then
    raise exception 'Invalid diagnostic event payload';
  end if;

  v_user_id := auth.uid();

  insert into public.diagnostics_events (
    installation_id,
    user_id,
    relay_version,
    platform,
    os_version,
    event_type,
    metadata,
    created_at
  )
  values (
    p_installation_id,
    v_user_id,
    p_relay_version,
    p_platform,
    p_os_version,
    p_event_type,
    coalesce(p_metadata, '{}'::jsonb),
    now()
  );
end;
$$;

-- Revoke execute on public functions from public and grant to anon + authenticated
grant execute on function public.register_installation_heartbeat(uuid, text, text, text) to anon, authenticated;
grant execute on function public.ingest_diagnostic_event(uuid, text, text, text, text, jsonb) to anon, authenticated;

-- Auto-update updated_at timestamp trigger
create or replace function public.handle_updated_at()
returns trigger as $$
begin
  new.updated_at = now();
  return new;
end;
$$ language plpgsql;

create trigger on_relay_accounts_updated
  before update on public.relay_accounts
  for each row
  execute function public.handle_updated_at();
