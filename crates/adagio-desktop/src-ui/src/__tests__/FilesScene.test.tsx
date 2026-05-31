import './mocks/tauri';
import { render, screen } from '@testing-library/react';
import FilesScene from '../components/FilesScene';

const BASE_PROPS = {
  pairId: 'pair-1',
  serverHost: 'cloud.example.com',
  currentPath: '/',
  onPathChange: vi.fn(),
  onShare: vi.fn(),
  syncStatus: null,
};

describe('FilesScene — E2EE lock icon (T050)', () => {
  beforeEach(() => vi.clearAllMocks());

  it('shows shield icon when isE2eePair=true', () => {
    render(<FilesScene {...BASE_PROPS} isE2eePair={true} />);
    // The shield icon renders as an SVG inside a span with title "End-to-end encrypted"
    expect(screen.getByTitle('End-to-end encrypted')).toBeInTheDocument();
  });

  it('does not show shield icon when isE2eePair=false', () => {
    render(<FilesScene {...BASE_PROPS} isE2eePair={false} />);
    expect(screen.queryByTitle('End-to-end encrypted')).not.toBeInTheDocument();
  });

  it('does not show shield icon when isE2eePair is omitted', () => {
    render(<FilesScene {...BASE_PROPS} />);
    expect(screen.queryByTitle('End-to-end encrypted')).not.toBeInTheDocument();
  });
});
