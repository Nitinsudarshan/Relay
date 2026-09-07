import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { NativeSidebar } from './NativeSidebar';
import type { MainTabType } from '@/types';

describe('NativeSidebar', () => {
  const defaultProps = {
    isOpen: true,
    onToggle: vi.fn(),
    activeTab: 'talkback' as MainTabType,
    setActiveTab: vi.fn(),
    account: null,
    profile: null,
    appVersion: '0.41.0',
    onOpenChangelog: vi.fn(),
    onOpenWelcome: vi.fn(),
    onOpenExplanation: vi.fn(),
  };

  it('renders primary destinations in expected order and does not include Home or capture actions', () => {
    render(<NativeSidebar {...defaultProps} />);

    // Primary items in requested order: Voice notes, Scribbles, Meetings, Talkback, Knowledge Graph
    const buttons = screen.getAllByRole('button');
    const buttonLabels = buttons.map((b) => b.getAttribute('aria-label')).filter(Boolean);
    const expectedOrder = [
      'Voice Notes',
      'Scribbles',
      'Meetings',
      'Talkback',
      'Knowledge Graph',
    ];
    let lastIdx = -1;
    for (const label of expectedOrder) {
      const idx = buttonLabels.indexOf(label);
      expect(idx).toBeGreaterThan(lastIdx);
      lastIdx = idx;
    }

    // System items
    expect(screen.getByRole('button', { name: 'Settings' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Diagnostics' })).toBeInTheDocument();

    // Must NOT have Home or capture methods as sidebar navigation items
    expect(screen.queryByRole('button', { name: 'Home' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Files' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Files & Docs' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Captures' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Web Capture' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Capture' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Typed Text' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Clipboard' })).not.toBeInTheDocument();
  });

  it('clicking the Relay logo in expanded mode navigates to Home', async () => {
    const setActiveTab = vi.fn();
    const user = userEvent.setup();
    render(<NativeSidebar {...defaultProps} isOpen={true} setActiveTab={setActiveTab} />);

    const logoButton = screen.getByRole('button', { name: 'Relay Home' });
    expect(logoButton).toBeInTheDocument();
    await user.click(logoButton);

    expect(setActiveTab).toHaveBeenCalledWith('home');
  });

  it('clicking the Relay logo in collapsed mode navigates to Home', async () => {
    const setActiveTab = vi.fn();
    const user = userEvent.setup();
    render(<NativeSidebar {...defaultProps} isOpen={false} setActiveTab={setActiveTab} />);

    const logoButton = screen.getByRole('button', { name: 'Relay Home' });
    expect(logoButton).toBeInTheDocument();
    await user.click(logoButton);

    expect(setActiveTab).toHaveBeenCalledWith('home');
  });

  it('navigates to primary and system destinations when clicked', async () => {
    const setActiveTab = vi.fn();
    const user = userEvent.setup();
    render(<NativeSidebar {...defaultProps} setActiveTab={setActiveTab} />);

    await user.click(screen.getByRole('button', { name: 'Talkback' }));
    expect(setActiveTab).toHaveBeenCalledWith('talkback');

    await user.click(screen.getByRole('button', { name: 'Voice Notes' }));
    expect(setActiveTab).toHaveBeenCalledWith('capture');

    await user.click(screen.getByRole('button', { name: 'Meetings' }));
    expect(setActiveTab).toHaveBeenCalledWith('meetings');

    await user.click(screen.getByRole('button', { name: 'Scribbles' }));
    expect(setActiveTab).toHaveBeenCalledWith('scribble');

    await user.click(screen.getByRole('button', { name: 'Knowledge Graph' }));
    expect(setActiveTab).toHaveBeenCalledWith('graph');

    await user.click(screen.getByRole('button', { name: 'Settings' }));
    expect(setActiveTab).toHaveBeenCalledWith('settings');

    await user.click(screen.getByRole('button', { name: 'Diagnostics' }));
    expect(setActiveTab).toHaveBeenCalledWith('diagnostics');
  });
});
