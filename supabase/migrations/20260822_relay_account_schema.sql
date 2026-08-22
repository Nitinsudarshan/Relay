-- ============================================================================
-- Relay Cloud Backend Schema & Row Level Security (RLS)
-- Supports Relay Identity, Installation Tracking, Telemetry, and App Updates
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

-- 4. APP RELEASES & UPDATES TABLE
create table if not exists public.app_releases (
  id uuid default gen_random_uuid() primary key,
  version text unique not null,
  min_supported_version text not null default '0.8.0',
  release_notes text,
  download_url text,
  is_active boolean default true,
  published_at timestamp with time zone default timezone('utc'::text, now()) not null
);

-- Seed current version
insert into public.app_releases (version, min_supported_version, release_notes, download_url, is_active)
values 
  ('0.9.0', '0.8.0', 'Phase 11D — Relay Identity, Product Account & Supabase Foundation', 'https://github.com/Nitinsudarshan/Relay/releases/tag/v0.9.0', true)
on conflict (version) do update set 
  release_notes = excluded.release_notes,
  is_active = excluded.is_active;

-- ============================================================================
-- ROW LEVEL SECURITY (RLS) POLICIES
-- ============================================================================

alter table public.relay_accounts enable row level security;
alter table public.installations enable row level security;
alter table public.diagnostics_events enable row level security;
alter table public.app_releases enable row level security;

-- Relay Accounts Policies
create policy "Users can view own account" on public.relay_accounts
  for select using (auth.uid() = id);

create policy "Users can update own account" on public.relay_accounts
  for update using (auth.uid() = id);

create policy "Users can insert own account" on public.relay_accounts
  for insert with check (auth.uid() = id);

-- Installations Policies (insert/upsert by anon or authenticated user)
create policy "Allow upsert installations" on public.installations
  for all using (true) with check (true);

-- Diagnostics Events Policies (insert-only)
create policy "Allow insert diagnostics" on public.diagnostics_events
  for insert with check (true);

create policy "Admins can view diagnostics" on public.diagnostics_events
  for select using (auth.role() = 'service_role');

-- App Releases Policies (public read)
create policy "Public can view active releases" on public.app_releases
  for select using (is_active = true);

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
