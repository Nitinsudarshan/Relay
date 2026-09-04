import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MeetingPipelineDiagnostics } from './MeetingPipelineDiagnostics';
import type { MeetingSelfTestReport } from '../../types';

const mockedInvoke = vi.mocked(invoke);

const report = (overrides: Partial<MeetingSelfTestReport> = {}): MeetingSelfTestReport => ({
  checks: [
    {
      id: 'gate_rejects_room_tone',
      name: 'Room tone is not decoded',
      purpose: 'Thirty seconds of steady background noise must never reach Whisper.',
      passed: true,
      detail: '0.00s voiced of 30s; overall RMS 0.0061, noise floor 0.0058',
      duration_ms: 14,
    },
    {
      id: 'loop_is_rejected',
      name: 'A decoder loop is discarded',
      purpose: 'One phrase repeated for the whole window must be thrown away.',
      passed: true,
      detail: 'rejected — decoder loop: "thank you" repeated 73 times',
      duration_ms: 2,
    },
  ],
  passed: 2,
  failed: 0,
  duration_ms: 42,
  whisper_checked: false,
  whisper_on_silence: null,
  ...overrides,
});

beforeEach(() => {
  mockedInvoke.mockReset();
});

/**
 * The panel exists because the bug it covers is machine-dependent. These tests
 * check that a user can actually read the verdict — including the measurement
 * behind it, which is what makes a green result trustworthy and a red one
 * diagnosable.
 */
describe('MeetingPipelineDiagnostics', () => {
  it('does not run anything until asked', () => {
    render(<MeetingPipelineDiagnostics />);
    expect(screen.getByText(/not run yet/i)).toBeInTheDocument();
    expect(mockedInvoke).not.toHaveBeenCalled();
  });

  it('runs the checks and shows each verdict with its measurement', async () => {
    mockedInvoke.mockResolvedValue(report());
    render(<MeetingPipelineDiagnostics />);

    await userEvent.click(screen.getByRole('button', { name: /run checks/i }));

    await waitFor(() =>
      expect(mockedInvoke).toHaveBeenCalledWith('run_meeting_pipeline_selftest'),
    );
    expect(await screen.findByText('Room tone is not decoded')).toBeInTheDocument();
    expect(screen.getByText(/0.00s voiced of 30s/)).toBeInTheDocument();
    expect(screen.getByText(/2\/2 passed/)).toBeInTheDocument();
    expect(screen.getAllByLabelText('passed')).toHaveLength(2);
  });

  it('shows a failing check as failing', async () => {
    mockedInvoke.mockResolvedValue(
      report({
        checks: [
          {
            id: 'gate_rejects_room_tone',
            name: 'Room tone is not decoded',
            purpose: 'Steady noise must never reach Whisper.',
            passed: false,
            detail: '8.40s voiced of 30s — the gate let room tone through',
            duration_ms: 12,
          },
        ],
        passed: 0,
        failed: 1,
      }),
    );
    render(<MeetingPipelineDiagnostics />);
    await userEvent.click(screen.getByRole('button', { name: /run checks/i }));

    expect(await screen.findByLabelText('failed')).toBeInTheDocument();
    expect(screen.getByText(/let room tone through/)).toBeInTheDocument();
    expect(screen.getByText(/0\/1 passed/)).toBeInTheDocument();
  });

  it('shows what the installed Whisper model invented on room tone', async () => {
    // The most useful line in the report: the user's own model, on this
    // machine, producing the exact failure the pipeline exists to catch.
    mockedInvoke.mockResolvedValue(
      report({
        whisper_checked: true,
        whisper_on_silence: 'Thank you. Thank you. Thank you.',
      }),
    );
    render(<MeetingPipelineDiagnostics />);
    await userEvent.click(screen.getByRole('button', { name: /run checks/i }));

    expect(
      await screen.findByText('Thank you. Thank you. Thank you.'),
    ).toBeInTheDocument();
    expect(screen.getByText(/hallucination the pipeline exists to catch/)).toBeInTheDocument();
  });

  it('says the Whisper checks were skipped rather than implying they passed', async () => {
    mockedInvoke.mockResolvedValue(report({ whisper_checked: false }));
    render(<MeetingPipelineDiagnostics />);
    await userEvent.click(screen.getByRole('button', { name: /run checks/i }));

    expect(
      await screen.findByText(/no whisper model is configured/i),
    ).toBeInTheDocument();
  });

  it('explains what a check proves when it is expanded', async () => {
    mockedInvoke.mockResolvedValue(report());
    render(<MeetingPipelineDiagnostics />);
    await userEvent.click(screen.getByRole('button', { name: /run checks/i }));

    const row = await screen.findByRole('button', { name: /room tone is not decoded/i });
    expect(
      screen.queryByText(/must never reach Whisper/),
    ).not.toBeInTheDocument();
    await userEvent.click(row);
    expect(screen.getByText(/must never reach Whisper/)).toBeInTheDocument();
  });

  it('reports a failure to run rather than showing a stale result', async () => {
    mockedInvoke.mockRejectedValue({ message: 'Whisper model could not be loaded' });
    render(<MeetingPipelineDiagnostics />);
    await userEvent.click(screen.getByRole('button', { name: /run checks/i }));

    expect(
      await screen.findByText('Whisper model could not be loaded'),
    ).toBeInTheDocument();
  });
});
