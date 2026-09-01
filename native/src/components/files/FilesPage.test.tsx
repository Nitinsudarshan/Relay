import { describe, test, expect, vi, beforeEach } from 'vitest';
import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { FilesPage } from './FilesPage';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn()
}));

const mockFiles = [
  {
    id: 'file_1',
    original_filename: 'Project_Proposal.pdf',
    file_type: 'pdf',
    mime_type: 'application/pdf',
    size_bytes: 1048576,
    content_hash: 'hash123',
    created_at: '2026-09-01T10:00:00Z',
    updated_at: '2026-09-01T10:00:00Z',
    last_known_source_path: 'C:\\Docs\\Project_Proposal.pdf',
    vault_path: 'files/file_1/original/Project_Proposal.pdf',
    extraction_status: 'extracted',
    processing_status: 'ready',
    content: 'Project proposal contents for local vault integration.',
    summary: '1. **Core Insight:** Project proposal details.',
    tags: ['proposal', 'planning'],
    topics: ['Architecture', 'Planning'],
    entities: ['Relay', 'Vault'],
    relationships: [],
    ai_metadata: {
      enrichment_status: 'enriched',
      last_enriched_at: '2026-09-01T10:05:00Z',
      suggested_questions: ['What is the core insight?']
    }
  }
];

describe('FilesPage Component', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (invoke as any).mockImplementation((cmd: string) => {
      if (cmd === 'get_vault_files') return Promise.resolve(mockFiles);
      if (cmd === 'import_vault_file') return Promise.resolve(mockFiles[0]);
      if (cmd === 'summarize_vault_file') return Promise.resolve(mockFiles[0]);
      if (cmd === 'delete_vault_file') return Promise.resolve(undefined);
      return Promise.resolve(null);
    });
  });

  test('renders Files Vault header and file cards', async () => {
    render(<FilesPage />);

    expect(screen.getByText('Files Vault')).toBeInTheDocument();

    await waitFor(() => {
      expect(screen.getByText('Project_Proposal.pdf')).toBeInTheDocument();
    });

    expect(screen.getByText('1 MB')).toBeInTheDocument();
    expect(screen.getByText('Architecture')).toBeInTheDocument();
  });

  test('rejects unsupported file formats with error banner', async () => {
    render(<FilesPage />);

    await waitFor(() => {
      expect(screen.getByText('Project_Proposal.pdf')).toBeInTheDocument();
    });

    // Simulate drag over and drop of unsupported .exe file
    const dropzone = screen.getByText(/Drag and drop documents here/i);
    const dropEvent = {
      preventDefault: vi.fn(),
      dataTransfer: {
        files: [{ name: 'executable.exe', path: 'C:\\executable.exe' }]
      }
    };

    fireEvent.drop(dropzone, dropEvent);

    await waitFor(() => {
      expect(screen.getByText(/Format .exe is not supported/i)).toBeInTheDocument();
    });
  });
});
