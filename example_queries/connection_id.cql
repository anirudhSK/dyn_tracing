MATCH (a)-[]->(b)-[]->(c) WHERE c.node.metadata.WORKLOAD_NAME = 'ratings-v1' RETURN a.connection.id
