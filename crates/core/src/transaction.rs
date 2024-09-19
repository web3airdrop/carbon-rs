use crate::{
    collection::InstructionDecoderCollection,
    error::CarbonResult,
    instruction::NestedInstruction,
    processor::Processor,
    schema::{ParsedInstruction, TransactionSchema},
};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use solana_sdk::{pubkey::Pubkey, signature::Signature};

#[derive(Debug, Clone)]
pub struct TransactionMetadata {
    pub slot: u64,
    pub signature: Signature,
    pub fee_payer: Pubkey,
}

pub struct TransactionPipe<T: InstructionDecoderCollection, U> {
    schema: TransactionSchema<T>,
    processor: Box<dyn Processor<InputType = U> + Send + Sync>,
}

pub struct ParsedTransaction<I: InstructionDecoderCollection> {
    pub metadata: TransactionMetadata,
    pub instructions: Vec<ParsedInstruction<I>>,
}

impl<T: InstructionDecoderCollection, U> TransactionPipe<T, U> {
    pub fn new(
        schema: TransactionSchema<T>,
        processor: impl Processor<InputType = U> + Send + Sync + 'static,
    ) -> Self {
        Self {
            schema,
            processor: Box::new(processor),
        }
    }

    fn parse_instructions(&self, instructions: &[NestedInstruction]) -> Vec<ParsedInstruction<T>> {
        instructions
            .iter()
            .filter_map(|nested_instr| {
                T::parse_instruction(nested_instr.instruction.clone()).map(|decoded_instruction| {
                    ParsedInstruction {
                        program_id: nested_instr.instruction.program_id,
                        instruction: decoded_instruction,
                        inner_instructions: self
                            .parse_instructions(&nested_instr.inner_instructions),
                    }
                })
            })
            .collect()
    }

    fn matches_schema(&self, instructions: &[ParsedInstruction<T>]) -> Option<U>
    where
        U: DeserializeOwned,
    {
        self.schema.match_schema(instructions)
    }
}

#[async_trait]
pub trait TransactionPipes {
    async fn run(&self, instructions: Vec<NestedInstruction>) -> CarbonResult<()>;
}

#[async_trait]
impl<T, U> TransactionPipes for TransactionPipe<T, U>
where
    T: InstructionDecoderCollection + Sync + 'static,
    U: DeserializeOwned + Send + Sync + 'static,
{
    async fn run(&self, instructions: Vec<NestedInstruction>) -> CarbonResult<()> {
        let parsed_instructions = self.parse_instructions(&instructions);

        for parsed in &parsed_instructions {
            println!("\nParsed ix: {:?}", parsed);
        }

        if let Some(matched_data) = self.matches_schema(&parsed_instructions) {
            self.processor.process(matched_data).await?;
        }

        Ok(())
    }
}
