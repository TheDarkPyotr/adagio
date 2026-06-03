import './mocks/tauri';
import { render, screen, fireEvent } from '@testing-library/react';
import Chrome from '../components/Chrome';

describe('Chrome', () => {
  it('renders the adagio wordmark', () => {
    render(<Chrome><div /></Chrome>);
    expect(screen.getByText('adagio')).toBeInTheDocument();
  });

  it('renders the search bar hint text', () => {
    render(<Chrome><div /></Chrome>);
    expect(screen.getByText('Search files, people, activity…')).toBeInTheDocument();
  });

  it('shows Files and Activity tabs when tab prop is provided', () => {
    render(<Chrome tab="files" onTab={vi.fn()}><div /></Chrome>);
    expect(screen.getByText('Files')).toBeInTheDocument();
    expect(screen.getByText('Activity')).toBeInTheDocument();
  });

  it('calls onTab with files when Files tab is clicked', () => {
    const onTab = vi.fn();
    render(<Chrome tab="activity" onTab={onTab}><div /></Chrome>);
    fireEvent.click(screen.getByText('Files'));
    expect(onTab).toHaveBeenCalledWith('files');
  });

  it('calls onTab with activity when Activity tab is clicked', () => {
    const onTab = vi.fn();
    render(<Chrome tab="files" onTab={onTab}><div /></Chrome>);
    fireEvent.click(screen.getByText('Activity'));
    expect(onTab).toHaveBeenCalledWith('activity');
  });

  it('does not render tab toggle when tab prop is omitted', () => {
    render(<Chrome><div /></Chrome>);
    expect(screen.queryByText('Files')).toBeNull();
    expect(screen.queryByText('Activity')).toBeNull();
  });

  it('renders children inside the layout', () => {
    render(<Chrome><span data-testid="child">hello</span></Chrome>);
    expect(screen.getByTestId('child')).toBeInTheDocument();
  });

  // T027 — conflict badge appears with count when pendingConflicts > 0.
  it('renders conflict badge with count when pendingConflicts > 0', () => {
    render(<Chrome pendingConflicts={3} onOpenConflicts={vi.fn()}><div /></Chrome>);
    expect(screen.getByTestId('conflict-badge')).toBeInTheDocument();
    expect(screen.getByTestId('conflict-badge')).toHaveTextContent('3');
  });

  // T028 — conflict badge is absent when pendingConflicts is 0.
  it('does not render conflict badge when pendingConflicts is 0', () => {
    render(<Chrome pendingConflicts={0}><div /></Chrome>);
    expect(screen.queryByTestId('conflict-badge')).toBeNull();
  });

  // Additional: clicking the badge calls onOpenConflicts.
  it('calls onOpenConflicts when conflict badge is clicked', () => {
    const onOpenConflicts = vi.fn();
    render(<Chrome pendingConflicts={2} onOpenConflicts={onOpenConflicts}><div /></Chrome>);
    fireEvent.click(screen.getByTestId('conflict-badge'));
    expect(onOpenConflicts).toHaveBeenCalledTimes(1);
  });
});
