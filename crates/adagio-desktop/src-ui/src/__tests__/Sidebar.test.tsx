import './mocks/tauri';
import { render, screen, fireEvent } from '@testing-library/react';
import Sidebar from '../components/Sidebar';
import type { SidebarAccount } from '../components/Sidebar';

const ACCOUNT: SidebarAccount = {
  id: 'a1',
  name: 'Alice Nextcloud',
  host: 'cloud.example.com',
  initial: 'A',
  color: 'var(--forest)',
};

const BASE_PROPS = {
  selected: 'all' as const,
  onSelect: vi.fn(),
  accounts: [ACCOUNT],
  activeAccountId: 'a1',
  onSwitchAccount: vi.fn(),
  onAddAccount: vi.fn(),
  onRemoveAccount: vi.fn(),
  pairs: [],
  syncStatus: null,
};

describe('Sidebar', () => {
  beforeEach(() => vi.clearAllMocks());

  it('renders the active account name', () => {
    render(<Sidebar {...BASE_PROPS} />);
    expect(screen.getByText('Alice Nextcloud')).toBeInTheDocument();
  });

  it('renders the account host', () => {
    render(<Sidebar {...BASE_PROPS} />);
    expect(screen.getByText('cloud.example.com')).toBeInTheDocument();
  });

  it('renders all library nav items', () => {
    render(<Sidebar {...BASE_PROPS} />);
    expect(screen.getByText('All files')).toBeInTheDocument();
    expect(screen.getByText('Favorites')).toBeInTheDocument();
    expect(screen.getByText('Recent')).toBeInTheDocument();
    expect(screen.getByText('Shared')).toBeInTheDocument();
    expect(screen.getByText('Tagged')).toBeInTheDocument();
  });

  it('calls onSelect with correct id when a nav item is clicked', () => {
    const onSelect = vi.fn();
    render(<Sidebar {...BASE_PROPS} onSelect={onSelect} />);
    fireEvent.click(screen.getByText('Favorites'));
    expect(onSelect).toHaveBeenCalledWith('fav');
  });

  it('calls onSelect with recent when Recent is clicked', () => {
    const onSelect = vi.fn();
    render(<Sidebar {...BASE_PROPS} onSelect={onSelect} />);
    fireEvent.click(screen.getByText('Recent'));
    expect(onSelect).toHaveBeenCalledWith('recent');
  });

  it('opens account dropdown when account picker is clicked', () => {
    render(<Sidebar {...BASE_PROPS} />);
    // Click the account picker button to open dropdown
    fireEvent.click(screen.getAllByRole('button')[0]);
    expect(screen.getByText('Add account')).toBeInTheDocument();
  });

  it('calls onAddAccount when Add account is clicked', () => {
    const onAddAccount = vi.fn();
    render(<Sidebar {...BASE_PROPS} onAddAccount={onAddAccount} />);
    fireEvent.click(screen.getAllByRole('button')[0]);
    fireEvent.click(screen.getByText('Add account'));
    expect(onAddAccount).toHaveBeenCalledOnce();
  });

  it('shows No folders yet when pairs is empty', () => {
    render(<Sidebar {...BASE_PROPS} pairs={[]} />);
    expect(screen.getByText('No folders yet')).toBeInTheDocument();
  });

  it('renders pinned folder name when pairs are provided', () => {
    render(<Sidebar {...BASE_PROPS} pairs={[{
      id: 'p1',
      account_id: 'a1',
      local_root: '/home/alice/Adagio',
      remote_root: '/',
      scan_interval_secs: 7200,
      scan_on_startup: true,
    }]} />);
    expect(screen.getByText('Adagio')).toBeInTheDocument();
  });
});
