import { render } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import ProtectedRoute from '../ProtectedRoute';
import { useAuthStore } from '../../store/auth';

// Mock the auth store
vi.mock('../../store/auth');

const mockUseAuthStore = vi.mocked(useAuthStore);

describe('ProtectedRoute', () => {
  it('redirects to /login when not authenticated', () => {
    // When selector is called with state, return false for isAuthenticated
    mockUseAuthStore.mockImplementation((selector) =>
      selector({
        isAuthenticated: false,
        setAuthenticated: vi.fn(),
        login: vi.fn(),
        logout: vi.fn(),
      }),
    );

    const { container } = render(
      <MemoryRouter initialEntries={['/']}>
        <ProtectedRoute>
          <div>Protected content</div>
        </ProtectedRoute>
      </MemoryRouter>,
    );

    // Should not render children
    expect(container.textContent).not.toContain('Protected content');
  });

  it('renders children when authenticated', () => {
    mockUseAuthStore.mockImplementation((selector) =>
      selector({
        isAuthenticated: true,
        setAuthenticated: vi.fn(),
        login: vi.fn(),
        logout: vi.fn(),
      }),
    );

    const { getByText } = render(
      <MemoryRouter initialEntries={['/']}>
        <ProtectedRoute>
          <div>Protected content</div>
        </ProtectedRoute>
      </MemoryRouter>,
    );

    expect(getByText('Protected content')).toBeInTheDocument();
  });
});
