import { getProgramId, getSystemProgramId } from './config';
import protocolIdl from '../../idl/tally-protocol.json';

export interface IdlConfig {
  programId?: string;
  systemProgramId?: string;
}

/**
 * Get the IDL with the correct program ID
 * @param config Optional configuration to override program ID and system program ID
 * @returns IDL object with the correct program ID
 */
export function getIdl(config?: IdlConfig) {
  const programId = getProgramId(config?.programId);
  const systemProgramId = getSystemProgramId(config?.systemProgramId);

  return {
    ...protocolIdl,
    address: programId,
    // Update system program addresses in accounts if needed
    instructions: protocolIdl.instructions.map(instruction => ({
      ...instruction,
      accounts: instruction.accounts.map(account => {
        // Update system program address if it's hardcoded
        if ('address' in account && account.address === '11111111111111111111111111111111' && account.name === 'system_program') {
          return {
            ...account,
            address: systemProgramId
          };
        }
        return account;
      })
    }))
  };
}

export { protocolIdl as rawIdl };