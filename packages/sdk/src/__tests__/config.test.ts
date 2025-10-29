import { getProgramId, getSystemProgramId } from '../config';

describe('Configuration', () => {
  const originalEnv = process.env;

  beforeEach(() => {
    // Reset process.env
    process.env = { ...originalEnv };
  });

  afterAll(() => {
    // Restore original environment
    process.env = originalEnv;
  });

  describe('getProgramId', () => {
    it('should return override when provided', () => {
      const testProgramId = 'So11111111111111111111111111111111111111112';
      const result = getProgramId(testProgramId);
      expect(result).toBe(testProgramId);
    });

    it('should return environment variable when set', () => {
      const testProgramId = 'So11111111111111111111111111111111111111112';
      process.env.TALLY_PROGRAM_ID = testProgramId;
      const result = getProgramId();
      expect(result).toBe(testProgramId);
    });

    it('should throw error when no override or env var', () => {
      delete process.env.TALLY_PROGRAM_ID;
      expect(() => getProgramId()).toThrow('TALLY_PROGRAM_ID environment variable is required');
    });

    it('should prioritize override over environment variable', () => {
      const envProgramId = 'So11111111111111111111111111111111111111112';
      const overrideProgramId = 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA';

      process.env.TALLY_PROGRAM_ID = envProgramId;
      const result = getProgramId(overrideProgramId);

      expect(result).toBe(overrideProgramId);
    });
  });

  describe('getSystemProgramId', () => {
    it('should return override when provided', () => {
      const testSystemProgramId = 'So11111111111111111111111111111111111111112';
      const result = getSystemProgramId(testSystemProgramId);
      expect(result).toBe(testSystemProgramId);
    });

    it('should return environment variable when set', () => {
      const testSystemProgramId = 'So11111111111111111111111111111111111111112';
      process.env.SYSTEM_PROGRAM_ID = testSystemProgramId;
      const result = getSystemProgramId();
      expect(result).toBe(testSystemProgramId);
    });

    it('should return default when no override or env var', () => {
      delete process.env.SYSTEM_PROGRAM_ID;
      const result = getSystemProgramId();
      expect(result).toBe('11111111111111111111111111111111');
    });
  });
});