import './mocks/tauri';
import { render, screen, fireEvent } from '@testing-library/react';
import OnboardingWizard from '../components/OnboardingWizard';

describe('OnboardingWizard', () => {
  const onComplete = vi.fn();
  beforeEach(() => vi.clearAllMocks());

  it('starts at step 1 and renders Welcome content', () => {
    render(<OnboardingWizard onComplete={onComplete} />);
    expect(screen.getByText('Welcome.')).toBeInTheDocument();
  });

  it('shows the step rail with all 5 step names', () => {
    render(<OnboardingWizard onComplete={onComplete} />);
    expect(screen.getByText('Welcome')).toBeInTheDocument();
    expect(screen.getByText('Server')).toBeInTheDocument();
    expect(screen.getByText('Authorize')).toBeInTheDocument();
    expect(screen.getByText('Where to sync')).toBeInTheDocument();
    expect(screen.getByText('Begin')).toBeInTheDocument();
  });

  it('shows step 1 of 5 indicator', () => {
    render(<OnboardingWizard onComplete={onComplete} />);
    expect(screen.getByText('step 1 of 5')).toBeInTheDocument();
  });

  it('advances to step 2 when Continue is clicked', () => {
    render(<OnboardingWizard onComplete={onComplete} />);
    fireEvent.click(screen.getByText('Continue'));
    expect(screen.getByText('step 2 of 5')).toBeInTheDocument();
  });

  it('shows a Server URL input on step 2', () => {
    render(<OnboardingWizard onComplete={onComplete} />);
    fireEvent.click(screen.getByText('Continue'));
    expect(screen.getByDisplayValue(/cloud\./)).toBeInTheDocument();
  });

  it('returns to step 1 when Back is clicked from step 2', () => {
    render(<OnboardingWizard onComplete={onComplete} />);
    fireEvent.click(screen.getByText('Continue'));
    fireEvent.click(screen.getByText('← Back'));
    expect(screen.getByText('Welcome.')).toBeInTheDocument();
  });

  it('preserves the URL input when navigating back and forward', () => {
    render(<OnboardingWizard onComplete={onComplete} />);
    fireEvent.click(screen.getByText('Continue'));
    const input = screen.getByDisplayValue(/cloud\./);
    fireEvent.change(input, { target: { value: 'https://my.cloud.io' } });
    fireEvent.click(screen.getByText('← Back'));
    fireEvent.click(screen.getByText('Continue'));
    expect(screen.getByDisplayValue('https://my.cloud.io')).toBeInTheDocument();
  });

  it('can navigate through all 5 steps', () => {
    render(<OnboardingWizard onComplete={onComplete} />);
    for (let i = 1; i < 5; i++) {
      fireEvent.click(screen.getByText('Continue'));
    }
    expect(screen.getByText('step 5 of 5')).toBeInTheDocument();
  });

  it('calls onComplete when Continue is clicked on step 5', () => {
    render(<OnboardingWizard onComplete={onComplete} />);
    for (let i = 1; i <= 5; i++) {
      const btn = screen.getByRole('button', { name: /Continue|Open Adagio/i });
      fireEvent.click(btn);
    }
    expect(onComplete).toHaveBeenCalledOnce();
  });
});
