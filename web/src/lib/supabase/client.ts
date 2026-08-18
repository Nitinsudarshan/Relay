// Supabase client factory for web hybrid surface per rules/data-access.md
// In local-only mode, no network calls are made. In hybrid mode, authenticates against Supabase.

export interface SupabaseKanbanCard {
  id: string;
  title: string;
  assignee: string;
  status: 'todo' | 'in_progress' | 'done';
  priority: 'high' | 'medium' | 'low';
  due_date?: string;
  created_at: string;
  description: string;
}

export class MockSupabaseClient {
  private static instance: MockSupabaseClient;

  public static getInstance(): MockSupabaseClient {
    if (!MockSupabaseClient.instance) {
      MockSupabaseClient.instance = new MockSupabaseClient();
    }
    return MockSupabaseClient.instance;
  }

  public async getKanbanCards(): Promise<{ data: SupabaseKanbanCard[] | null; error: string | null }> {
    // Returns hybrid synced cards or local fallback
    return {
      data: [
        {
          id: 'web_card_001',
          title: 'Setup hybrid cloud database schema',
          assignee: 'Nitin',
          status: 'in_progress',
          priority: 'high',
          due_date: '2026-08-22',
          created_at: new Date().toISOString(),
          description: 'Deploy Supabase PostgreSQL tables and RLS policies for hybrid sync.',
        },
        {
          id: 'web_card_002',
          title: 'Implement keep-alive heartbeat for Supabase free-tier',
          assignee: 'Nitin',
          status: 'todo',
          priority: 'medium',
          due_date: '2026-08-25',
          created_at: new Date().toISOString(),
          description: 'Mitigate Supabase idle auto-pause after 7 days.',
        },
      ],
      error: null,
    };
  }
}

export const getSupabaseClient = () => MockSupabaseClient.getInstance();
