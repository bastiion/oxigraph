//! Utilities to read RDF graphs and datasets

use crate::error::invalid_data_error;
use crate::io::{DatasetFormat, GraphFormat};
use crate::model::*;
use oxiri::{Iri, IriParseError};
use rio_api::model as rio;
use rio_api::parser::{QuadsParser, TriplesParser};
use rio_turtle::{NQuadsParser, NTriplesParser, TriGParser, TurtleParser};
use rio_xml::RdfXmlParser;
use std::collections::HashMap;
use std::io;
use std::io::BufRead;

/// A reader for RDF graph serialization formats.
///
/// It currently supports the following formats:
/// * [N-Triples](https://www.w3.org/TR/n-triples/) (`GraphFormat::NTriples`)
/// * [Turtle](https://www.w3.org/TR/turtle/) (`GraphFormat::Turtle`)
/// * [RDF XML](https://www.w3.org/TR/rdf-syntax-grammar/) (`GraphFormat::RdfXml`)
///
/// ```
/// use oxigraph::io::{GraphFormat, GraphParser};
/// use std::io::Cursor;
///
/// let file = "<http://example.com/s> <http://example.com/p> <http://example.com/o> .";
///
/// let parser = GraphParser::from_format(GraphFormat::NTriples);
/// let triples = parser.read_triples(Cursor::new(file))?.collect::<Result<Vec<_>,_>>()?;
///
///assert_eq!(triples.len(), 1);
///assert_eq!(triples[0].subject.to_string(), "<http://example.com/s>");
/// # std::io::Result::Ok(())
/// ```
pub struct GraphParser {
    format: GraphFormat,
    base_iri: String,
}

impl GraphParser {
    pub fn from_format(format: GraphFormat) -> Self {
        Self {
            format,
            base_iri: String::new(),
        }
    }

    /// Provides an IRI that could be used to resolve the file relative IRIs
    ///
    /// ```
    /// use oxigraph::io::{GraphFormat, GraphParser};
    /// use std::io::Cursor;
    ///
    /// let file = "</s> </p> </o> .";
    ///
    /// let parser = GraphParser::from_format(GraphFormat::Turtle).with_base_iri("http://example.com")?;
    /// let triples = parser.read_triples(Cursor::new(file))?.collect::<Result<Vec<_>,_>>()?;
    ///
    ///assert_eq!(triples.len(), 1);
    ///assert_eq!(triples[0].subject.to_string(), "<http://example.com/s>");
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn with_base_iri(mut self, base_iri: impl Into<String>) -> Result<Self, IriParseError> {
        self.base_iri = Iri::parse(base_iri.into())?.into_inner();
        Ok(self)
    }

    /// Executes the parsing itself
    pub fn read_triples<R: BufRead>(&self, reader: R) -> Result<TripleReader<R>, io::Error> {
        //TODO: drop the error when possible
        Ok(TripleReader {
            mapper: RioMapper::default(),
            parser: match self.format {
                GraphFormat::NTriples => TripleReaderKind::NTriples(
                    NTriplesParser::new(reader).map_err(invalid_data_error)?,
                ),
                GraphFormat::Turtle => TripleReaderKind::Turtle(
                    TurtleParser::new(reader, &self.base_iri).map_err(invalid_data_error)?,
                ),
                GraphFormat::RdfXml => TripleReaderKind::RdfXml(
                    RdfXmlParser::new(reader, &self.base_iri).map_err(invalid_data_error)?,
                ),
            },
            buffer: Vec::new(),
        })
    }
}

/// Allows reading triples.
/// Could be built using a `GraphParser`.
///
/// ```
/// use oxigraph::io::{GraphFormat, GraphParser};
/// use std::io::Cursor;
///
/// let file = "<http://example.com/s> <http://example.com/p> <http://example.com/o> .";
///
/// let parser = GraphParser::from_format(GraphFormat::NTriples);
/// let triples = parser.read_triples(Cursor::new(file))?.collect::<Result<Vec<_>,_>>()?;
///
///assert_eq!(triples.len(), 1);
///assert_eq!(triples[0].subject.to_string(), "<http://example.com/s>");
/// # std::io::Result::Ok(())
/// ```
#[must_use]
pub struct TripleReader<R: BufRead> {
    mapper: RioMapper,
    parser: TripleReaderKind<R>,
    buffer: Vec<Triple>,
}

enum TripleReaderKind<R: BufRead> {
    NTriples(NTriplesParser<R>),
    Turtle(TurtleParser<R>),
    RdfXml(RdfXmlParser<R>),
}

impl<R: BufRead> Iterator for TripleReader<R> {
    type Item = Result<Triple, io::Error>;

    fn next(&mut self) -> Option<Result<Triple, io::Error>> {
        loop {
            if let Some(r) = self.buffer.pop() {
                return Some(Ok(r));
            }

            if let Err(error) = match &mut self.parser {
                TripleReaderKind::NTriples(parser) => Self::read(
                    parser,
                    &mut self.buffer,
                    &mut self.mapper,
                    invalid_data_error,
                ),
                TripleReaderKind::Turtle(parser) => Self::read(
                    parser,
                    &mut self.buffer,
                    &mut self.mapper,
                    invalid_data_error,
                ),
                TripleReaderKind::RdfXml(parser) => Self::read(
                    parser,
                    &mut self.buffer,
                    &mut self.mapper,
                    invalid_data_error,
                ),
            }? {
                return Some(Err(error));
            }
        }
    }
}

impl<R: BufRead> TripleReader<R> {
    fn read<P: TriplesParser>(
        parser: &mut P,
        buffer: &mut Vec<Triple>,
        mapper: &mut RioMapper,
        error: impl Fn(P::Error) -> io::Error,
    ) -> Option<Result<(), io::Error>> {
        if parser.is_end() {
            None
        } else if let Err(e) = parser.parse_step(&mut |t| {
            buffer.push(mapper.triple(&t));
            Ok(())
        }) {
            Some(Err(error(e)))
        } else {
            Some(Ok(()))
        }
    }
}

/// A reader for RDF dataset serialization formats.
///
/// It currently supports the following formats:
/// * [N-Quads](https://www.w3.org/TR/n-quads/) (`DatasetFormat::NQuads`)
/// * [TriG](https://www.w3.org/TR/trig/) (`DatasetFormat::TriG`)
///
/// ```
/// use oxigraph::io::{DatasetFormat, DatasetParser};
/// use std::io::Cursor;
///
/// let file = "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .";
///
/// let parser = DatasetParser::from_format(DatasetFormat::NQuads);
/// let quads = parser.read_quads(Cursor::new(file))?.collect::<Result<Vec<_>,_>>()?;
///
///assert_eq!(quads.len(), 1);
///assert_eq!(quads[0].subject.to_string(), "<http://example.com/s>");
/// # std::io::Result::Ok(())
/// ```
pub struct DatasetParser {
    format: DatasetFormat,
    base_iri: String,
}

impl DatasetParser {
    pub fn from_format(format: DatasetFormat) -> Self {
        Self {
            format,
            base_iri: String::new(),
        }
    }

    /// Provides an IRI that could be used to resolve the file relative IRIs
    ///
    /// ```
    /// use oxigraph::io::{DatasetFormat, DatasetParser};
    /// use std::io::Cursor;
    ///
    /// let file = "<g> { </s> </p> </o> }";
    ///
    /// let parser = DatasetParser::from_format(DatasetFormat::TriG).with_base_iri("http://example.com")?;
    /// let triples = parser.read_quads(Cursor::new(file))?.collect::<Result<Vec<_>,_>>()?;
    ///
    ///assert_eq!(triples.len(), 1);
    ///assert_eq!(triples[0].subject.to_string(), "<http://example.com/s>");
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn with_base_iri(mut self, base_iri: impl Into<String>) -> Result<Self, IriParseError> {
        self.base_iri = Iri::parse(base_iri.into())?.into_inner();
        Ok(self)
    }

    /// Executes the parsing itself
    pub fn read_quads<R: BufRead>(&self, reader: R) -> Result<QuadReader<R>, io::Error> {
        //TODO: drop the error when possible
        Ok(QuadReader {
            mapper: RioMapper::default(),
            parser: match self.format {
                DatasetFormat::NQuads => {
                    QuadReaderKind::NQuads(NQuadsParser::new(reader).map_err(invalid_data_error)?)
                }
                DatasetFormat::TriG => QuadReaderKind::TriG(
                    TriGParser::new(reader, &self.base_iri).map_err(invalid_data_error)?,
                ),
            },
            buffer: Vec::new(),
        })
    }
}

/// Allows reading quads.
/// Could be built using a `DatasetParser`.
///
/// ```
/// use oxigraph::io::{DatasetFormat, DatasetParser};
/// use std::io::Cursor;
///
/// let file = "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .";
///
/// let parser = DatasetParser::from_format(DatasetFormat::NQuads);
/// let quads = parser.read_quads(Cursor::new(file))?.collect::<Result<Vec<_>,_>>()?;
///
///assert_eq!(quads.len(), 1);
///assert_eq!(quads[0].subject.to_string(), "<http://example.com/s>");
/// # std::io::Result::Ok(())
/// ```
#[must_use]
pub struct QuadReader<R: BufRead> {
    mapper: RioMapper,
    parser: QuadReaderKind<R>,
    buffer: Vec<Quad>,
}

enum QuadReaderKind<R: BufRead> {
    NQuads(NQuadsParser<R>),
    TriG(TriGParser<R>),
}

impl<R: BufRead> Iterator for QuadReader<R> {
    type Item = Result<Quad, io::Error>;

    fn next(&mut self) -> Option<Result<Quad, io::Error>> {
        loop {
            if let Some(r) = self.buffer.pop() {
                return Some(Ok(r));
            }

            if let Err(error) = match &mut self.parser {
                QuadReaderKind::NQuads(parser) => Self::read(
                    parser,
                    &mut self.buffer,
                    &mut self.mapper,
                    invalid_data_error,
                ),
                QuadReaderKind::TriG(parser) => Self::read(
                    parser,
                    &mut self.buffer,
                    &mut self.mapper,
                    invalid_data_error,
                ),
            }? {
                return Some(Err(error));
            }
        }
    }
}

impl<R: BufRead> QuadReader<R> {
    fn read<P: QuadsParser>(
        parser: &mut P,
        buffer: &mut Vec<Quad>,
        mapper: &mut RioMapper,
        error: impl Fn(P::Error) -> io::Error,
    ) -> Option<Result<(), io::Error>> {
        if parser.is_end() {
            None
        } else if let Err(e) = parser.parse_step(&mut |t| {
            buffer.push(mapper.quad(&t));
            Ok(())
        }) {
            Some(Err(error(e)))
        } else {
            Some(Ok(()))
        }
    }
}

#[derive(Default)]
struct RioMapper {
    bnode_map: HashMap<String, BlankNode>,
}

impl<'a> RioMapper {
    fn named_node(&self, node: rio::NamedNode<'a>) -> NamedNode {
        NamedNode::new_unchecked(node.iri)
    }

    fn blank_node(&mut self, node: rio::BlankNode<'a>) -> BlankNode {
        self.bnode_map
            .entry(node.id.to_owned())
            .or_insert_with(BlankNode::default)
            .clone()
    }

    fn literal(&self, literal: rio::Literal<'a>) -> Literal {
        match literal {
            rio::Literal::Simple { value } => Literal::new_simple_literal(value),
            rio::Literal::LanguageTaggedString { value, language } => {
                Literal::new_language_tagged_literal_unchecked(value, language)
            }
            rio::Literal::Typed { value, datatype } => {
                Literal::new_typed_literal(value, self.named_node(datatype))
            }
        }
    }

    fn named_or_blank_node(&mut self, node: rio::NamedOrBlankNode<'a>) -> NamedOrBlankNode {
        match node {
            rio::NamedOrBlankNode::NamedNode(node) => self.named_node(node).into(),
            rio::NamedOrBlankNode::BlankNode(node) => self.blank_node(node).into(),
        }
    }

    fn term(&mut self, node: rio::Term<'a>) -> Term {
        match node {
            rio::Term::NamedNode(node) => self.named_node(node).into(),
            rio::Term::BlankNode(node) => self.blank_node(node).into(),
            rio::Term::Literal(literal) => self.literal(literal).into(),
        }
    }

    fn triple(&mut self, triple: &rio::Triple<'a>) -> Triple {
        Triple {
            subject: self.named_or_blank_node(triple.subject),
            predicate: self.named_node(triple.predicate),
            object: self.term(triple.object),
        }
    }

    fn graph_name(&mut self, graph_name: Option<rio::NamedOrBlankNode<'a>>) -> GraphName {
        match graph_name {
            Some(rio::NamedOrBlankNode::NamedNode(node)) => self.named_node(node).into(),
            Some(rio::NamedOrBlankNode::BlankNode(node)) => self.blank_node(node).into(),
            None => GraphName::DefaultGraph,
        }
    }

    fn quad(&mut self, quad: &rio::Quad<'a>) -> Quad {
        Quad {
            subject: self.named_or_blank_node(quad.subject),
            predicate: self.named_node(quad.predicate),
            object: self.term(quad.object),
            graph_name: self.graph_name(quad.graph_name),
        }
    }
}
